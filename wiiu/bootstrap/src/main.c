#include <stdint.h>
#include <stddef.h>

#include <gx2/draw.h>
#include <gx2/shaders.h>
#include <gx2r/buffer.h>
#include <gx2r/draw.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern void horizon_wiiu_frame_color(uint32_t frame, float rgba[4]);
extern uint32_t horizon_wiiu_build_block_mesh(float *vertices, uint32_t capacity);
extern const uint8_t _binary_horizon_triangle_gsh_start[];

#define BLOCK_VERTEX_COUNT 18

typedef struct Vertex {
   float position[3];
   float color[3];
} Vertex;

static WHBGfxShaderGroup sBlockShader;
static GX2RBuffer sBlockBuffer;

static BOOL
init_block_mesh(void)
{
   if (!WHBGfxLoadGFDShaderGroup(&sBlockShader, 0, _binary_horizon_triangle_gsh_start)) {
      return FALSE;
   }

   if (!WHBGfxInitShaderAttribute(&sBlockShader, "aPosition", 0,
                                  offsetof(Vertex, position), GX2_ATTRIB_FORMAT_FLOAT_32_32_32)
       || !WHBGfxInitShaderAttribute(&sBlockShader, "aColor", 0,
                                     offsetof(Vertex, color), GX2_ATTRIB_FORMAT_FLOAT_32_32_32)
       || !WHBGfxInitFetchShader(&sBlockShader)) {
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }

   sBlockBuffer.flags = GX2R_RESOURCE_BIND_VERTEX_BUFFER
                        | GX2R_RESOURCE_USAGE_CPU_READ
                        | GX2R_RESOURCE_USAGE_CPU_WRITE
                        | GX2R_RESOURCE_USAGE_GPU_READ;
   sBlockBuffer.elemSize = sizeof(Vertex);
   sBlockBuffer.elemCount = BLOCK_VERTEX_COUNT;

   if (!GX2RCreateBuffer(&sBlockBuffer)) {
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }

   void *buffer = GX2RLockBufferEx(&sBlockBuffer, 0);
   if (!buffer) {
      GX2RDestroyBufferEx(&sBlockBuffer, sBlockBuffer.flags);
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }

   uint32_t vertexCount = horizon_wiiu_build_block_mesh((float *)buffer, BLOCK_VERTEX_COUNT);
   GX2RUnlockBufferEx(&sBlockBuffer, 0);
   if (vertexCount != BLOCK_VERTEX_COUNT) {
      GX2RDestroyBufferEx(&sBlockBuffer, sBlockBuffer.flags);
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }
   return TRUE;
}

static void
draw_block_mesh(void)
{
   GX2SetVertexShader(sBlockShader.vertexShader);
   GX2SetPixelShader(sBlockShader.pixelShader);
   GX2SetFetchShader(&sBlockShader.fetchShader);
   GX2RSetAttributeBuffer(&sBlockBuffer, 0, sizeof(Vertex), 0);
   GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, BLOCK_VERTEX_COUNT, 0, 1);
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

   if (!init_block_mesh()) {
      WHBGfxShutdown();
      WHBProcShutdown();
      return 2;
   }

   uint32_t frame = 0;
   while (WHBProcIsRunning()) {
      float rgba[4];
      horizon_wiiu_frame_color(frame++, rgba);

      WHBGfxBeginRender();
      WHBGfxBeginRenderTV();
      WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
      draw_block_mesh();
      WHBGfxFinishRenderTV();

      WHBGfxBeginRenderDRC();
      WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
      draw_block_mesh();
      WHBGfxFinishRenderDRC();
      WHBGfxFinishRender();
   }

   free_block_mesh();
   WHBGfxShutdown();
   WHBProcShutdown();
   return 0;
}
