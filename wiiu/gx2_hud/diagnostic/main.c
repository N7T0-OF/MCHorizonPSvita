/* Explicit test fixture for the real GX2 HUD presenter; it contains no game assets. */
#include "../hud.h"
#include <coreinit/debug.h>
#include <coreinit/filesystem.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdio.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>
#include <whb/sdcard.h>

extern const unsigned char _binary_hud_diag_gsh_start[];
static FSClient sClient;
static FSCmdBlock sCmd;
static int sFileReady;
static _Alignas(64) char sLog[512];

static void status(const char *message)
{
   OSReport("Horizon HUD diagnostic: %s\n", message);
   if (!sFileReady) return;
   char path[256];
   snprintf(path, sizeof(path), "%s/wiiu/apps/HorizonWiiU/hud-diagnostic-status.log", WHBGetSdCardMountPath());
   FSFileHandle file;
   if (FSOpenFile(&sClient, &sCmd, path, "w", &file, FS_ERROR_FLAG_NONE) == FS_STATUS_OK) {
      snprintf(sLog, sizeof(sLog), "%s\n", message);
      FSWriteFile(&sClient, &sCmd, (uint8_t *)sLog, 1, (uint32_t)strlen(sLog), file, 0, FS_ERROR_FLAG_NONE);
      FSCloseFile(&sClient, &sCmd, file, FS_ERROR_FLAG_NONE);
   }
}

static void quad(HorizonHudVertex *v, float x0, float y0, float x1, float y1,
                 float u, uint32_t color)
{
   const float points[6][2] = {{x0,y0},{x1,y0},{x1,y1},{x0,y0},{x1,y1},{x0,y1}};
   for (unsigned i = 0; i < 6; ++i) {
      v[i] = (HorizonHudVertex){.pos = {points[i][0], points[i][1]}, .uv = {u, 0.5f}, .color = color};
   }
}

static int readback(char *message, size_t capacity)
{
   GX2Surface *src = &WHBGfxGetTVColourBuffer()->surface;
   GX2Surface dst = {0};
   dst.dim = GX2_SURFACE_DIM_TEXTURE_2D;
   dst.width = src->width; dst.height = src->height; dst.depth = 1;
   dst.mipLevels = 1; dst.format = src->format; dst.aa = GX2_AA_MODE1X;
   /* Cemu 2.6 triggers a texture readback for this diagnostic-only copy. */
   dst.tileMode = GX2_TILE_MODE_LINEAR_SPECIAL;
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE | GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&dst, flags)) {
      snprintf(message, capacity, "readback surface allocation failed");
      return 0;
   }
   GX2DrawDone();
   GX2CopySurface(src, 0, 0, &dst, 0, 0);
   GX2DrawDone();
   const uint8_t *p = GX2RLockSurfaceEx(&dst, 0, flags);
   if (!p) {
      GX2RDestroySurfaceEx(&dst, flags);
      snprintf(message, capacity, "readback surface lock failed");
      return 0;
   }
   const uint8_t *red = p + ((size_t)64 * dst.pitch + 64) * 4;
   const uint8_t *green = p + ((size_t)64 * dst.pitch + 224) * 4;
   const uint8_t *background = p + ((size_t)8 * dst.pitch + 8) * 4;
   const int ok = red[0] > 200 && red[1] < 30 && red[2] < 30 &&
                  green[0] < 30 && green[1] > 200 && green[2] < 30 &&
                  background[0] > 30 && background[0] < 80;
   snprintf(message, capacity,
            "readback=%s target=%ux%u format=0x%x tiles=%u,%u pitch=%u red=%u,%u,%u,%u green=%u,%u,%u,%u background=%u,%u,%u,%u",
            ok ? "pass" : "mismatch", src->width, src->height, src->format, src->tileMode, dst.tileMode, dst.pitch,
            red[0],red[1],red[2],red[3],green[0],green[1],green[2],green[3],
            background[0],background[1],background[2],background[3]);
   GX2RUnlockSurfaceEx(&dst, 0, flags);
   GX2RDestroySurfaceEx(&dst, flags);
   return ok;
}

int main(void)
{
   WHBProcInit();
   if (WHBMountSdCard() && FSAddClient(&sClient, FS_ERROR_FLAG_NONE) == FS_STATUS_OK) {
      FSInitCmdBlock(&sCmd);
      sFileReady = 1;
   }
   status("entry reached");
   if (!WHBGfxInit()) { status("GX2 initialization failed"); return 1; }
   const uint8_t atlas[] = {0,0,0,255, 255,255,255,255};
   const uint8_t white[] = {255,255,255,255};
   HorizonGx2Hud *hud = NULL;
   HorizonHudResult result = horizon_gx2_hud_create(&hud, _binary_hud_diag_gsh_start, 2, 1, atlas, sizeof(atlas), 12);
   if (result != HORIZON_HUD_OK) {
      char report[96];
      snprintf(report, sizeof(report), "presenter create failed=%d", result);
      status(report);
      WHBGfxShutdown();
      if (sFileReady) FSDelClient(&sClient, FS_ERROR_FLAG_NONE);
      WHBUnmountSdCard(); WHBProcShutdown();
      return 2;
   }
   HorizonHudVertex vertices[12];
   quad(vertices, 32, 32, 160, 160, 0.25f, UINT32_C(0xff0000ff));
   quad(vertices + 6, 192, 32, 320, 160, 0.75f, UINT32_C(0xff00ff00));
   HorizonHudRun runs[] = {{0,6,HORIZON_HUD_ALPHA}, {6,12,HORIZON_HUD_ALPHA}};
   HorizonHudPatch patch = {0,0,1,1,white,sizeof(white)};
   GX2Surface *tv = &WHBGfxGetTVColourBuffer()->surface;
   HorizonHudFrame frame = {vertices,12,runs,2,&patch,1,tv->width,tv->height,1.0f,0};
   runs[0].blend = HORIZON_HUD_INVERT;
   if (horizon_gx2_hud_draw(hud, &frame) != HORIZON_HUD_UNSUPPORTED) {
      status("unsupported blend not rejected");
      horizon_gx2_hud_destroy(hud); WHBGfxShutdown();
      if (sFileReady) FSDelClient(&sClient, FS_ERROR_FLAG_NONE);
      WHBUnmountSdCard(); WHBProcShutdown();
      return 3;
   }
   runs[0].blend = HORIZON_HUD_ALPHA;
   int verified = 0;
   if (WHBProcIsRunning()) {
      WHBGfxBeginRender(); WHBGfxBeginRenderTV();
      WHBGfxClearColor(0.2f, 0.2f, 0.2f, 1.0f);
      result = horizon_gx2_hud_draw(hud, &frame);
      char report[384];
      if (result == HORIZON_HUD_OK) verified = readback(report, sizeof(report));
      else snprintf(report, sizeof(report), "presenter draw failed=%d", result);
      WHBGfxFinishRenderTV(); WHBGfxFinishRender();
      status(report);
   } else status("process not in foreground");
   horizon_gx2_hud_destroy(hud);
   WHBGfxShutdown();
   if (sFileReady) FSDelClient(&sClient, FS_ERROR_FLAG_NONE);
   WHBUnmountSdCard();
   WHBProcShutdown();
   return verified ? 0 : 4;
}
