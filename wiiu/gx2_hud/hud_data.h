#pragma once
#include "hud.h"

typedef struct HorizonHudGpuVertex {
   float position[4], color[4], uv[2];
} HorizonHudGpuVertex;

int horizon_hud_rgba_bytes(uint32_t width, uint32_t height, size_t *bytes);
HorizonHudResult horizon_hud_validate(const HorizonHudFrame *frame, uint32_t atlas_width,
                                       uint32_t atlas_height, uint32_t capacity);
void horizon_hud_encode_vertices(const HorizonHudFrame *frame, HorizonHudGpuVertex *output);
