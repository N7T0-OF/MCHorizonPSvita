import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest


TOOL = Path(__file__).resolve().parents[1] / "tools" / "horizon_wiiu_capture.py"
SPEC = importlib.util.spec_from_file_location("horizon_wiiu_capture", TOOL)
capture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(capture)


def vertex(position, uv=(0, 0), layer=0, light=0, color=0, endian="<"):
    return struct.pack(f"{endian}fffHHHHI", *position, *uv, layer, light, color)


class CaptureConversionTests(unittest.TestCase):
    def test_selects_nearest_section_and_emits_host_decoded_vertices(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            records = [
                {"key": [10, 0, 0], "solid": 4, "translucent": 1},
                {"key": [0, 4, 0], "solid": 4, "translucent": 0},
            ]
            (root / "sections.json").write_text(json.dumps(records), encoding="utf-8")
            (root / "scene.json").write_text(
                json.dumps({"frame": {"camera": [0.0, 64.0, 0.0]}, "renderer": {"min_y": 0}}),
                encoding="utf-8",
            )
            vertices = [vertex([index, 2.0, 3.0], (12, 13), 7, 0x1234, 0xA1B2C3D4) for index in range(4)]
            translucent = vertex([99.0, 99.0, 99.0])
            (root / "sections.bin").write_bytes(b"".join(vertices + [translucent] + vertices))

            output = root / "generated.c"
            result = capture.convert(root, output, source_endian="little")
            generated = output.read_text(encoding="ascii")

            self.assertEqual(result["key"], (0, 4, 0))
            self.assertEqual(result["origin"], [0.0, 0.0, 0.0])
            self.assertEqual(result["vertices"], 4)
            self.assertIn("0x1.8000000000000p+1f", generated)
            self.assertIn("12u, 13u", generated)
            self.assertIn("7u, 4660u, 0xA1B2C3D4u", generated)

    def test_rejects_truncated_capture_and_oversized_nearest_section(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            (root / "sections.json").write_text(
                json.dumps([{"key": [0, 0, 0], "solid": 4, "translucent": 0}]),
                encoding="utf-8",
            )
            (root / "scene.json").write_text(
                json.dumps({"frame": {"camera": [0.0, 0.0, 0.0]}, "renderer": {"min_y": 0}}),
                encoding="utf-8",
            )
            (root / "sections.bin").write_bytes(b"short")

            with self.assertRaisesRegex(ValueError, "describe"):
                capture.convert(root, root / "generated.c")

            (root / "sections.json").write_text(
                json.dumps([{"key": [0, 0, 0], "solid": 8, "translucent": 0}]),
                encoding="utf-8",
            )
            (root / "sections.bin").write_bytes(vertex([0.0, 0.0, 0.0]) * 8)
            with self.assertRaisesRegex(ValueError, "increase --max-vertices"):
                capture.convert(root, root / "generated.c", max_vertices=4)

    def test_decodes_big_endian_capture_records(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            (root / "sections.json").write_text(
                json.dumps([{"key": [0, 0, 0], "solid": 4, "translucent": 0}]),
                encoding="utf-8",
            )
            (root / "scene.json").write_text(
                json.dumps({"frame": {"camera": [8.0, 8.0, 8.0]}, "renderer": {"min_y": 0}}),
                encoding="utf-8",
            )
            (root / "sections.bin").write_bytes(
                b"".join(vertex([1.5, 2.5, 3.5], (17, 29), 9, 0x4567, 0x12345678, ">") for _ in range(4))
            )

            output = root / "generated.c"
            capture.convert(root, output, source_endian="big")
            generated = output.read_text(encoding="ascii")

            self.assertIn("0x1.8000000000000p+0f, 0x1.4000000000000p+1f, 0x1.c000000000000p+1f", generated)
            self.assertIn("17u, 29u", generated)
            self.assertIn("9u, 17767u, 0x12345678u", generated)


if __name__ == "__main__":
    unittest.main()
