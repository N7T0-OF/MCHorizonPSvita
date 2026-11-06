#!/usr/bin/env python3
"""Convert one Horizon render-capture section to a Wii U C vertex table.

The desktop capture stores native-endian `TerrainVertex` records. This tool
decodes their fields on the host and emits C constants so the Wii U compiler
lays them out in CafeOS's big-endian ABI.
"""

import argparse
import json
import math
from pathlib import Path
import struct
import sys


VERTEX_SIZE = 24
DEFAULT_MAX_VERTICES = 65_536
MAX_VERTICES = 4_000_000


def c_float(value):
    if not math.isfinite(value):
        raise ValueError("capture contains a non-finite terrain position")
    return f"{value.hex()}f"


def convert(capture_dir: Path, output: Path, max_vertices=DEFAULT_MAX_VERTICES,
            source_endian=sys.byteorder):
    if source_endian not in ("little", "big"):
        raise ValueError("source endian must be little or big")
    if not 4 <= max_vertices <= MAX_VERTICES:
        raise ValueError(f"max vertices must be between 4 and {MAX_VERTICES}")

    records = json.loads((capture_dir / "sections.json").read_text(encoding="utf-8"))
    scene = json.loads((capture_dir / "scene.json").read_text(encoding="utf-8"))
    camera = scene["frame"]["camera"]
    min_y = scene["renderer"]["min_y"]
    if len(camera) != 3 or not all(math.isfinite(float(value)) for value in camera):
        raise ValueError("scene camera must contain three finite coordinates")

    blob_path = capture_dir / "sections.bin"
    blob_size = blob_path.stat().st_size
    cursor = 0
    candidates = []
    for record in records:
        key = record["key"]
        solid_count = record["solid"]
        translucent_count = record["translucent"]
        if (len(key) != 3 or any(not isinstance(value, int) for value in key)
                or not isinstance(solid_count, int) or solid_count < 0
                or not isinstance(translucent_count, int) or translucent_count < 0):
            raise ValueError("section record has an invalid key or vertex count")
        if solid_count % 4:
            raise ValueError(f"section {key} solid mesh is not quad-aligned")
        solid_offset = cursor
        solid_bytes = solid_count * VERTEX_SIZE
        cursor += solid_bytes + translucent_count * VERTEX_SIZE

        origin = [
            key[0] * 16.0 - float(camera[0]),
            min_y + key[1] * 16.0 - float(camera[1]),
            key[2] * 16.0 - float(camera[2]),
        ]
        distance_squared = sum((value + 8.0) ** 2 for value in origin)
        if solid_count:
            candidates.append((distance_squared, tuple(key), solid_count, solid_offset, origin))

    if cursor != blob_size:
        raise ValueError(f"sections.bin has {blob_size} bytes; section records describe {cursor}")
    if not candidates:
        raise ValueError("capture contains no solid section meshes")

    candidates.sort(key=lambda item: (item[0], item[1]))
    selected = next((candidate for candidate in candidates if candidate[2] <= max_vertices), None)
    if selected is None:
        nearest = candidates[0]
        raise ValueError(
            f"nearest solid section {nearest[1]} has {nearest[2]} vertices; "
            f"increase --max-vertices (currently {max_vertices})"
        )

    _, key, vertex_count, offset, origin = selected
    with blob_path.open("rb") as stream:
        stream.seek(offset)
        data = stream.read(vertex_count * VERTEX_SIZE)
    if len(data) != vertex_count * VERTEX_SIZE:
        raise ValueError("sections.bin ended while reading the selected solid mesh")

    endian = "<" if source_endian == "little" else ">"
    vertex_format = struct.Struct(f"{endian}fffHHHHI")
    rows = []
    for index in range(vertex_count):
        position_x, position_y, position_z, u, v, layer, light, color = vertex_format.unpack_from(
            data, index * VERTEX_SIZE
        )
        position = (position_x, position_y, position_z)
        if not all(math.isfinite(value) for value in position):
            raise ValueError(f"section {key} vertex {index} has a non-finite position")
        rows.append(
            "   {{%s, %s, %s}, {%uu, %uu}, %uu, %uu, 0x%08Xu}," % (
                *(c_float(value) for value in position), u, v, layer, light, color
            )
        )

    output.parent.mkdir(parents=True, exist_ok=True)
    contents = "\n".join(
        [
            "#include <stdint.h>",
            "typedef struct HorizonTerrainVertex {",
            "   float position[3];",
            "   uint16_t uv[2];",
            "   uint16_t layer;",
            "   uint16_t light;",
            "   uint32_t color;",
            "} HorizonTerrainVertex;",
            '_Static_assert(sizeof(HorizonTerrainVertex) == 24, "TerrainVertex ABI must stay 24 bytes");',
            "",
            "const HorizonTerrainVertex horizon_wiiu_capture_vertices[] = {",
            *rows,
            "};",
            f"const uint32_t horizon_wiiu_capture_vertex_count = {vertex_count}u;",
            "const float horizon_wiiu_capture_section_origin[3] = {",
            f"   {c_float(origin[0])}, {c_float(origin[1])}, {c_float(origin[2])}",
            "};",
            "",
        ]
    )
    output.write_text(contents, encoding="ascii", newline="\n")
    return {"key": key, "vertices": vertex_count, "origin": origin, "output": output}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_dir", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--max-vertices", type=int, default=DEFAULT_MAX_VERTICES)
    parser.add_argument("--source-endian", choices=("little", "big"), default=sys.byteorder)
    args = parser.parse_args(argv)
    try:
        result = convert(args.capture_dir, args.output, args.max_vertices, args.source_endian)
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as error:
        parser.error(str(error))
    print(
        f"selected section {result['key']}: {result['vertices']} solid vertices, "
        f"camera-relative origin {result['origin']} -> {result['output']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
