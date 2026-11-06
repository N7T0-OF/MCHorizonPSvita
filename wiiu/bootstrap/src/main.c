#include <stdint.h>

#include <whb/gfx.h>
#include <whb/proc.h>

extern void horizon_wiiu_frame_color(uint32_t frame, float rgba[4]);

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

   uint32_t frame = 0;
   while (WHBProcIsRunning()) {
      float rgba[4];
      horizon_wiiu_frame_color(frame++, rgba);

      WHBGfxBeginRender();
      WHBGfxBeginRenderTV();
      WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
      WHBGfxFinishRenderTV();

      WHBGfxBeginRenderDRC();
      WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
      WHBGfxFinishRenderDRC();
      WHBGfxFinishRender();
   }

   WHBGfxShutdown();
   WHBProcShutdown();
   return 0;
}
