#include <stdint.h>
#include <stddef.h>
#include <string.h>

#include <gx2/draw.h>
#include <gx2/shaders.h>
#include <gx2r/buffer.h>
#include <gx2r/draw.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern void horizon_wiiu_frame_color(uint32_t frame, float rgba[4]);
extern const uint8_t _binary_horizon_triangle_gsh_start[];

typedef struct Vertex {
   float position[3];
   float color[3];
} Vertex;

static WHBGfxShaderGroup sTriangleShader;
static GX2RBuffer sTriangleBuffer;

static BOOL
init_triangle(void)
{
   static const Vertex vertices[] = {
      { {  0.0f,  0.68f, 0.0f }, { 1.00f, 0.24f, 0.12f } },
      { { -0.62f, -0.54f, 0.0f }, { 0.12f, 1.00f, 0.26f } },
      { {  0.62f, -0.54f, 0.0f }, { 0.18f, 0.38f, 1.00f } },
   };

   if (!WHBGfxLoadGFDShaderGroup(&sTriangleShader, 0, _binary_horizon_triangle_gsh_start)) {
      return FALSE;
   }

   if (!WHBGfxInitShaderAttribute(&sTriangleShader, "aPosition", 0,
                                  offsetof(Vertex, position), GX2_ATTRIB_FORMAT_FLOAT_32_32_32)
       || !WHBGfxInitShaderAttribute(&sTriangleShader, "aColor", 0,
                                     offsetof(Vertex, color), GX2_ATTRIB_FORMAT_FLOAT_32_32_32)
       || !WHBGfxInitFetchShader(&sTriangleShader)) {
      WHBGfxFreeShaderGroup(&sTriangleShader);
      return FALSE;
   }

   sTriangleBuffer.flags = GX2R_RESOURCE_BIND_VERTEX_BUFFER
                           | GX2R_RESOURCE_USAGE_CPU_READ
                           | GX2R_RESOURCE_USAGE_CPU_WRITE
                           | GX2R_RESOURCE_USAGE_GPU_READ;
   sTriangleBuffer.elemSize = sizeof(Vertex);
   sTriangleBuffer.elemCount = sizeof(vertices) / sizeof(vertices[0]);

   if (!GX2RCreateBuffer(&sTriangleBuffer)) {
      WHBGfxFreeShaderGroup(&sTriangleShader);
      return FALSE;
   }

   void *buffer = GX2RLockBufferEx(&sTriangleBuffer, 0);
   if (!buffer) {
      GX2RDestroyBufferEx(&sTriangleBuffer, sTriangleBuffer.flags);
      WHBGfxFreeShaderGroup(&sTriangleShader);
      return FALSE;
   }

   memcpy(buffer, vertices, sizeof(vertices));
   GX2RUnlockBufferEx(&sTriangleBuffer, 0);
   return TRUE;
}

static void
draw_triangle(void)
{
   GX2SetVertexShader(sTriangleShader.vertexShader);
   GX2SetPixelShader(sTriangleShader.pixelShader);
   GX2SetFetchShader(&sTriangleShader.fetchShader);
   GX2RSetAttributeBuffer(&sTriangleBuffer, 0, sizeof(Vertex), 0);
   GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, 3, 0, 1);
}

static void
free_triangle(void)
{
   if (GX2RBufferExists(&sTriangleBuffer)) {
      GX2RDestroyBufferEx(&sTriangleBuffer, sTriangleBuffer.flags);
   }
   WHBGfxFreeShaderGroup(&sTriangleShader);
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

   if (!init_triangle()) {
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
      draw_triangle();
      WHBGfxFinishRenderTV();

      WHBGfxBeginRenderDRC();
      WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
      draw_triangle();
      WHBGfxFinishRenderDRC();
      WHBGfxFinishRender();
   }

   free_triangle();
   WHBGfxShutdown();
   WHBProcShutdown();
   return 0;
}
