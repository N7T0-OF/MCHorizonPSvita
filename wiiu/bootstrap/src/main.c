#include <stdint.h>
#include <stddef.h>
#include <string.h>

#include <gx2/clear.h>
#include <gx2/draw.h>
#include <gx2/registers.h>
#include <gx2/sampler.h>
#include <gx2/shaders.h>
#include <gx2/texture.h>
#include <gx2r/buffer.h>
#include <gx2r/draw.h>
#include <gx2r/surface.h>
#include <vpad/input.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern void horizon_wiiu_frame_color(uint32_t frame, float rgba[4]);
extern const uint8_t _binary_horizon_block_gsh_start[];

#define BLOCK_VERTEX_COUNT 414
#define SECTION_VERTEX_COUNT 276
#define ATLAS_TILE_SIZE 4
#define ATLAS_SIZE (ATLAS_TILE_SIZE * 2)

typedef struct Vertex {
   float position[3];
   float color[3];
   float uv[2];
} Vertex;

typedef struct HorizonTerrainVertex {
   float position[3];
   uint16_t uv[2];
   uint16_t layer;
   uint16_t light;
   uint32_t color;
} HorizonTerrainVertex;

_Static_assert(sizeof(HorizonTerrainVertex) == 24, "Horizon TerrainVertex ABI must stay 24 bytes");

extern uint32_t horizon_wiiu_build_block_section(HorizonTerrainVertex *vertices,
                                                  uint32_t capacity);
extern uint32_t horizon_wiiu_encode_terrain_quads(const HorizonTerrainVertex *source,
                                                   uint32_t source_vertex_count,
                                                   float *output,
                                                   uint32_t output_capacity,
                                                   float section_x,
                                                   float section_y,
                                                   float section_z,
                                                   float camera_x,
                                                   float camera_z);

#ifdef HORIZON_WIIU_CAPTURE
extern const HorizonTerrainVertex horizon_wiiu_capture_vertices[];
extern const uint32_t horizon_wiiu_capture_vertex_count;
extern const float horizon_wiiu_capture_section_origin[3];
#endif

static WHBGfxShaderGroup sBlockShader;
static GX2RBuffer sBlockBuffer;
static GX2Texture sBlockAtlas;
static GX2Sampler sBlockSampler;
static const HorizonTerrainVertex *sBlockSection;
#ifndef HORIZON_WIIU_CAPTURE
static HorizonTerrainVertex sFixtureSection[SECTION_VERTEX_COUNT];
#endif
static float sSectionOrigin[3];
static uint32_t sBlockSectionVertexCount;
static uint32_t sBlockOutputCapacity;
static uint32_t sBlockVertexCount;
static float sCameraX;
static float sCameraZ;
static BOOL sBlockMeshDirty = TRUE;

static void
set_atlas_pixel(uint8_t *pixels, uint32_t pitch, uint32_t x, uint32_t y,
                uint8_t red, uint8_t green, uint8_t blue)
{
   uint8_t *pixel = pixels + ((y * pitch + x) * 4);
   pixel[0] = red;
   pixel[1] = green;
   pixel[2] = blue;
   pixel[3] = 255;
}

static BOOL
init_block_atlas(void)
{
   memset(&sBlockAtlas, 0, sizeof(sBlockAtlas));
   sBlockAtlas.surface.use = GX2_SURFACE_USE_TEXTURE;
   sBlockAtlas.surface.dim = GX2_SURFACE_DIM_TEXTURE_2D;
   sBlockAtlas.surface.width = ATLAS_SIZE;
   sBlockAtlas.surface.height = ATLAS_SIZE;
   sBlockAtlas.surface.depth = 1;
   sBlockAtlas.surface.mipLevels = 1;
   sBlockAtlas.surface.format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8;
   sBlockAtlas.surface.aa = GX2_AA_MODE1X;
   sBlockAtlas.surface.tileMode = GX2_TILE_MODE_LINEAR_ALIGNED;
   sBlockAtlas.viewNumMips = 1;
   sBlockAtlas.viewNumSlices = 1;
   // GX2 component selectors are three-bit fields in X/Y/Z/W order.
   sBlockAtlas.compMap = 0U | (1U << 3) | (2U << 6) | (3U << 9);

   const GX2RResourceFlags textureFlags = GX2R_RESOURCE_BIND_TEXTURE
                                          | GX2R_RESOURCE_USAGE_CPU_WRITE
                                          | GX2R_RESOURCE_USAGE_GPU_READ;
   if (!GX2RCreateSurface(&sBlockAtlas.surface, textureFlags)) {
      return FALSE;
   }

   uint8_t *pixels = (uint8_t *)GX2RLockSurfaceEx(&sBlockAtlas.surface, 0, textureFlags);
   if (!pixels) {
      GX2RDestroySurfaceEx(&sBlockAtlas.surface, textureFlags);
      memset(&sBlockAtlas, 0, sizeof(sBlockAtlas));
      return FALSE;
   }

   const uint32_t pitch = sBlockAtlas.surface.pitch;
   for (uint32_t y = 0; y < ATLAS_SIZE; ++y) {
      for (uint32_t x = 0; x < ATLAS_SIZE; ++x) {
         const uint32_t tileX = x / ATLAS_TILE_SIZE;
         const uint32_t tileY = y / ATLAS_TILE_SIZE;
         const uint32_t localX = x % ATLAS_TILE_SIZE;
         const uint32_t localY = y % ATLAS_TILE_SIZE;
         const BOOL speckle = ((localX + localY) & 1) != 0;
         uint8_t red, green, blue;

         if (tileX == 0 && tileY == 0) {
            red = speckle ? 72 : 55;
            green = speckle ? 220 : 199;
            blue = speckle ? 78 : 62;
         } else if (tileX == 1 && tileY == 0) {
            if (localY == 0) {
               red = speckle ? 72 : 55;
               green = speckle ? 192 : 171;
               blue = speckle ? 66 : 52;
            } else {
               red = speckle ? 177 : 154;
               green = speckle ? 109 : 88;
               blue = speckle ? 61 : 47;
            }
         } else if (tileX == 0) {
            red = speckle ? 119 : 101;
            green = speckle ? 110 : 94;
            blue = speckle ? 81 : 69;
         } else {
            red = speckle ? 133 : 116;
            green = speckle ? 87 : 72;
            blue = speckle ? 49 : 40;
         }

         set_atlas_pixel(pixels, pitch, x, y, red, green, blue);
      }
   }

   GX2RUnlockSurfaceEx(&sBlockAtlas.surface, 0, textureFlags);
   GX2InitTextureRegs(&sBlockAtlas);
   GX2InitSampler(&sBlockSampler, GX2_TEX_CLAMP_MODE_CLAMP, GX2_TEX_XY_FILTER_MODE_POINT);
   return TRUE;
}

static BOOL
init_block_mesh(void)
{
   if (!WHBGfxLoadGFDShaderGroup(&sBlockShader, 0, _binary_horizon_block_gsh_start)) {
      return FALSE;
   }

   if (!WHBGfxInitShaderAttribute(&sBlockShader, "aPosition", 0,
                                  offsetof(Vertex, position), GX2_ATTRIB_FORMAT_FLOAT_32_32_32)
       || !WHBGfxInitShaderAttribute(&sBlockShader, "aColor", 0,
                                     offsetof(Vertex, color), GX2_ATTRIB_FORMAT_FLOAT_32_32_32)
       || !WHBGfxInitShaderAttribute(&sBlockShader, "aUv", 0,
                                     offsetof(Vertex, uv), GX2_ATTRIB_FORMAT_FLOAT_32_32)
       || !WHBGfxInitFetchShader(&sBlockShader)) {
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }

#ifdef HORIZON_WIIU_CAPTURE
   sBlockSection = horizon_wiiu_capture_vertices;
   sBlockSectionVertexCount = horizon_wiiu_capture_vertex_count;
   memcpy(sSectionOrigin, horizon_wiiu_capture_section_origin, sizeof(sSectionOrigin));
   if (!sBlockSection || sBlockSectionVertexCount == 0 || sBlockSectionVertexCount % 4 != 0) {
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }
   sBlockOutputCapacity = sBlockSectionVertexCount / 4 * 6;
#else
   sBlockSection = sFixtureSection;
   sSectionOrigin[0] = -1.0f;
   sSectionOrigin[1] = 0.0f;
   sSectionOrigin[2] = -1.0f;
   sBlockSectionVertexCount = horizon_wiiu_build_block_section(sFixtureSection,
                                                                SECTION_VERTEX_COUNT);
   if (sBlockSectionVertexCount == 0 || sBlockSectionVertexCount > SECTION_VERTEX_COUNT) {
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }
   sBlockOutputCapacity = BLOCK_VERTEX_COUNT;
#endif

   if (sBlockOutputCapacity == 0 || sBlockOutputCapacity > UINT32_MAX / sizeof(Vertex)) {
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }

   sBlockBuffer.flags = GX2R_RESOURCE_BIND_VERTEX_BUFFER
                        | GX2R_RESOURCE_USAGE_CPU_READ
                        | GX2R_RESOURCE_USAGE_CPU_WRITE
                        | GX2R_RESOURCE_USAGE_GPU_READ;
   sBlockBuffer.elemSize = sizeof(Vertex);
   sBlockBuffer.elemCount = sBlockOutputCapacity;

   if (!GX2RCreateBuffer(&sBlockBuffer)) {
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }

   return TRUE;
}

static void
update_camera(void)
{
   const float previousCameraX = sCameraX;
   const float previousCameraZ = sCameraZ;
   VPADStatus status;
   VPADReadError error = VPAD_READ_SUCCESS;
   if (VPADRead(VPAD_CHAN_0, &status, 1, &error) <= 0 || error != VPAD_READ_SUCCESS) {
      return;
   }

   const float deadzone = 0.12f;
   if (status.leftStick.x > deadzone || status.leftStick.x < -deadzone) {
      sCameraX += status.leftStick.x * 0.025f;
   }
   if (status.leftStick.y > deadzone || status.leftStick.y < -deadzone) {
      sCameraZ -= status.leftStick.y * 0.025f;
   }
   if (status.hold & VPAD_BUTTON_LEFT) {
      sCameraX -= 0.025f;
   }
   if (status.hold & VPAD_BUTTON_RIGHT) {
      sCameraX += 0.025f;
   }
   if (status.hold & VPAD_BUTTON_UP) {
      sCameraZ += 0.025f;
   }
   if (status.hold & VPAD_BUTTON_DOWN) {
      sCameraZ -= 0.025f;
   }

   if (sCameraX < -4.0f) sCameraX = -4.0f;
   if (sCameraX > 4.0f) sCameraX = 4.0f;
   if (sCameraZ < -4.0f) sCameraZ = -4.0f;
   if (sCameraZ > 4.0f) sCameraZ = 4.0f;
   if (sCameraX != previousCameraX || sCameraZ != previousCameraZ) {
      sBlockMeshDirty = TRUE;
   }
}

static BOOL
update_block_mesh(void)
{
   void *buffer = GX2RLockBufferEx(&sBlockBuffer, 0);
   if (!buffer) {
      return FALSE;
   }

   uint32_t vertexCount = horizon_wiiu_encode_terrain_quads(sBlockSection,
                                                             sBlockSectionVertexCount,
                                                             (float *)buffer,
                                                             sBlockOutputCapacity,
                                                             sSectionOrigin[0],
                                                             sSectionOrigin[1],
                                                             sSectionOrigin[2],
                                                             sCameraX,
                                                             sCameraZ);
   GX2RUnlockBufferEx(&sBlockBuffer, 0);
   if (vertexCount == 0 || vertexCount > sBlockOutputCapacity) {
      return FALSE;
   }

   sBlockVertexCount = vertexCount;
   return TRUE;
}

static void
free_block_atlas(void)
{
   if (sBlockAtlas.surface.image) {
      const GX2RResourceFlags textureFlags = GX2R_RESOURCE_BIND_TEXTURE
                                             | GX2R_RESOURCE_USAGE_CPU_WRITE
                                             | GX2R_RESOURCE_USAGE_GPU_READ;
      GX2RDestroySurfaceEx(&sBlockAtlas.surface, textureFlags);
      memset(&sBlockAtlas, 0, sizeof(sBlockAtlas));
   }
}

static void
draw_block_mesh(void)
{
   GX2SetVertexShader(sBlockShader.vertexShader);
   GX2SetPixelShader(sBlockShader.pixelShader);
   GX2SetFetchShader(&sBlockShader.fetchShader);
   GX2RSetAttributeBuffer(&sBlockBuffer, 0, sizeof(Vertex), 0);
   GX2SetPixelTexture(&sBlockAtlas, 0);
   GX2SetPixelSampler(&sBlockSampler, 0);
   GX2SetDepthOnlyControl(TRUE, TRUE, GX2_COMPARE_FUNC_LEQUAL);
   GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, sBlockVertexCount, 0, 1);
}

static void
free_block_mesh(void)
{
   if (GX2RBufferExists(&sBlockBuffer)) {
      GX2RDestroyBufferEx(&sBlockBuffer, sBlockBuffer.flags);
   }
   WHBGfxFreeShaderGroup(&sBlockShader);
}

int
main(int argc, char **argv)
{
   (void)argc;
   (void)argv;

   WHBProcInit();
   if (!WHBGfxInit()) {
      WHBProcShutdown();
      return 1;
   }

   if (!init_block_atlas()) {
      WHBGfxShutdown();
      WHBProcShutdown();
      return 2;
   }

   if (!init_block_mesh()) {
      free_block_atlas();
      WHBGfxShutdown();
      WHBProcShutdown();
      return 3;
   }

   uint32_t frame = 0;
   while (WHBProcIsRunning()) {
      float rgba[4];
      horizon_wiiu_frame_color(frame++, rgba);
      update_camera();
      if (sBlockMeshDirty && update_block_mesh()) {
         sBlockMeshDirty = FALSE;
      }
      const BOOL hasMesh = sBlockVertexCount != 0;

      WHBGfxBeginRender();
      WHBGfxBeginRenderTV();
      WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
      if (hasMesh) draw_block_mesh();
      WHBGfxFinishRenderTV();

      WHBGfxBeginRenderDRC();
      WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
      if (hasMesh) draw_block_mesh();
      WHBGfxFinishRenderDRC();
      WHBGfxFinishRender();
   }

   free_block_mesh();
   free_block_atlas();
   WHBGfxShutdown();
   WHBProcShutdown();
   return 0;
}
