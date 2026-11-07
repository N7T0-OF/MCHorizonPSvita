//! Shared shader translation description for resource-pack block outlines.
use crate::Glue;

/// The outline's edges (`pack_lines::layout`: start, end, ARGB colour and width per instance) as
/// vanilla's lines: four vertices an edge, the start twice and the end twice, the side
/// `gl_VertexIndex % 2`, as `vs_line` draws them.
pub const GLUE: Glue = Glue {
    inputs: &[
        ("Position", "vec3"),
        ("Color", "vec4"),
        ("Normal", "vec3"),
        ("LineWidth", "float"),
    ],
    prelude: "layout(location = 0) in vec3 hz_start;\n\
              layout(location = 1) in vec3 hz_end;\n\
              layout(location = 2) in vec4 hz_argb;\n\
              layout(location = 3) in float hz_width;",
    // ARGB in a little-endian u32 reads as B, G, R, A. ShapeRenderer's normal: the direction.
    setup: "Position = hz_start; if (int(gl_VertexIndex) >= 2) Position = hz_end; \
            Color = hz_argb.zyxw; Normal = normalize(hz_end - hz_start); LineWidth = hz_width;",
    // 26.3's ProjMat already has wgpu's reversed 0 to 1 depth (Projection: zZeroToOne).
    finish: "",
    vertex_id: "int(gl_VertexIndex)",
};
