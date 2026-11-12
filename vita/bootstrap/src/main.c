/* Horizon Vita - bootstrap de diagnostic (AUCUN moteur Horizon ici). Code original, MIT.
 *
 * Ce binaire sert uniquement a valider sur console : ecran 960x544, boutons,
 * sticks, tactile, budget memoire et ecriture de fichiers. PLAY ne lance rien.
 */
#include <SDL2/SDL.h>
#include <psp2/ctrl.h>
#include <psp2/io/fcntl.h>
#include <psp2/io/stat.h>
#include <psp2/kernel/sysmem.h>
#include <psp2/touch.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define W 960
#define H 544
#define VERSION "0.0.2"
#define DATA_DIR "ux0:data/HorizonVita"
#define LOG_PATH DATA_DIR "/bootstrap.log"

/* newlib heap : valeur explicite pour que le budget soit connu et mesurable. */
unsigned int _newlib_heap_size_user = 32 * 1024 * 1024;

static const uint8_t GLYPHS[128][7] = {
  [32] = {0, 0, 0, 0, 0, 0, 0},
  [65] = {14, 17, 17, 31, 17, 17, 17},
  [66] = {30, 17, 17, 30, 17, 17, 30},
  [67] = {15, 16, 16, 16, 16, 16, 15},
  [68] = {30, 17, 17, 17, 17, 17, 30},
  [69] = {31, 16, 16, 30, 16, 16, 31},
  [70] = {31, 16, 16, 30, 16, 16, 16},
  [71] = {15, 16, 16, 23, 17, 17, 14},
  [72] = {17, 17, 17, 31, 17, 17, 17},
  [73] = {31, 4, 4, 4, 4, 4, 31},
  [74] = {7, 2, 2, 2, 18, 18, 12},
  [75] = {17, 18, 20, 24, 20, 18, 17},
  [76] = {16, 16, 16, 16, 16, 16, 31},
  [77] = {17, 27, 21, 21, 17, 17, 17},
  [78] = {17, 25, 21, 19, 17, 17, 17},
  [79] = {14, 17, 17, 17, 17, 17, 14},
  [80] = {30, 17, 17, 30, 16, 16, 16},
  [81] = {14, 17, 17, 17, 21, 18, 13},
  [82] = {30, 17, 17, 30, 20, 18, 17},
  [83] = {15, 16, 16, 14, 1, 1, 30},
  [84] = {31, 4, 4, 4, 4, 4, 4},
  [85] = {17, 17, 17, 17, 17, 17, 14},
  [86] = {17, 17, 17, 17, 17, 10, 4},
  [87] = {17, 17, 17, 21, 21, 21, 10},
  [88] = {17, 17, 10, 4, 10, 17, 17},
  [89] = {17, 17, 10, 4, 4, 4, 4},
  [90] = {31, 1, 2, 4, 8, 16, 31},
  [48] = {14, 19, 21, 21, 21, 25, 14},
  [49] = {4, 12, 4, 4, 4, 4, 14},
  [50] = {14, 17, 1, 2, 4, 8, 31},
  [51] = {30, 1, 1, 14, 1, 1, 30},
  [52] = {2, 6, 10, 18, 31, 2, 2},
  [53] = {31, 16, 16, 30, 1, 1, 30},
  [54] = {15, 16, 16, 30, 17, 17, 14},
  [55] = {31, 1, 2, 4, 8, 8, 8},
  [56] = {14, 17, 17, 14, 17, 17, 14},
  [57] = {14, 17, 17, 15, 1, 1, 30},
  [58] = {0, 4, 4, 0, 4, 4, 0},
  [46] = {0, 0, 0, 0, 0, 6, 6},
  [45] = {0, 0, 0, 31, 0, 0, 0},
  [47] = {1, 1, 2, 4, 8, 16, 16},
  [43] = {0, 4, 4, 31, 4, 4, 0},
};

static SceUID g_log = -1;

static void logf_(const char *fmt, ...) {
  char buf[256];
  va_list ap;
  va_start(ap, fmt);
  int n = vsnprintf(buf, sizeof buf - 1, fmt, ap);
  va_end(ap);
  if (n < 0) return;
  if (n > (int)sizeof buf - 2) n = (int)sizeof buf - 2;
  buf[n++] = '\n';
  if (g_log >= 0) sceIoWrite(g_log, buf, (SceSize)n);
}

static void fill(SDL_Renderer *r, int x, int y, int w, int h, int red, int green, int blue) {
  SDL_Rect rc = {x, y, w, h};
  SDL_SetRenderDrawColor(r, red, green, blue, 255);
  SDL_RenderFillRect(r, &rc);
}

static void letters(SDL_Renderer *r, const char *s, int x, int y, int scale, int cr, int cg, int cb) {
  while (*s) {
    unsigned char c = (unsigned char)*s++;
    if (c >= 'a' && c <= 'z') c = (unsigned char)(c - 32);
    if (c >= 128) c = ' ';
    for (int row = 0; row < 7; row++)
      for (int col = 0; col < 5; col++)
        if (GLYPHS[c][row] & (1 << (4 - col))) fill(r, x + col * scale, y + row * scale, scale, scale, cr, cg, cb);
    x += 6 * scale;
  }
}

static void textf(SDL_Renderer *r, int x, int y, int scale, int cr, int cg, int cb, const char *fmt, ...) {
  char buf[96];
  va_list ap;
  va_start(ap, fmt);
  vsnprintf(buf, sizeof buf, fmt, ap);
  va_end(ap);
  letters(r, buf, x, y, scale, cr, cg, cb);
}

typedef struct {
  SceCtrlData pad;
  SceTouchData front, back;
  int have_pad, have_front, have_back;
  SceKernelFreeMemorySizeInfo mem;
  int have_mem;
  uint32_t frame_ms; /* duree mesuree de la derniere frame, SDL_Delay inclus */
  int log_ok;
} Diag;

static void draw_header(SDL_Renderer *r) {
  SDL_SetRenderDrawColor(r, 13, 18, 25, 255);
  SDL_RenderClear(r);
  fill(r, 0, 0, W, 9, 46, 197, 169);
  fill(r, 0, 9, W, 95, 21, 32, 40);
  letters(r, "HORIZON VITA", 48, 36, 5, 239, 245, 248);
  letters(r, "DEV " VERSION, 690, 50, 2, 120, 183, 188);
  fill(r, 48, 127, 864, 46, 49, 39, 30);
  letters(r, "ENGINE NOT PORTED - BOOTSTRAP ONLY", 67, 143, 2, 255, 182, 91);
}

static void draw_menu(SDL_Renderer *r, int selected, int page) {
  draw_header(r);
  const char *items[] = {"PLAY", "MODS", "DIAGNOSTIC", "EXIT"};
  for (int i = 0; i < 4; i++) {
    int y = 201 + i * 65, on = selected == i;
    fill(r, 48, y, 864, 53, on ? 34 : 26, on ? 77 : 39, on ? 76 : 51);
    fill(r, 48, y, 6, 53, on ? 46 : 65, on ? 197 : 78, on ? 169 : 92);
    letters(r, items[i], 72, y + 14, 3, 238, 246, 245);
    if (i == 0) letters(r, "NOT READY", 644, y + 18, 2, 250, 183, 99);
    if (i == 1) letters(r, "NOT READY", 644, y + 18, 2, 170, 182, 190);
    if (i == 2) letters(r, "INPUT AND MEMORY", 644, y + 18, 2, 170, 182, 190);
  }
  fill(r, 48, 483, 864, 2, 65, 87, 95);
  if (page == 1) letters(r, "NO GAME YET - ENGINE NOT PORTED", 48, 502, 2, 252, 194, 116);
  else if (page == 2) letters(r, "MODS REQUIRE ENGINE PORT", 48, 502, 2, 252, 194, 116);
  else letters(r, "UP DOWN SELECT   CROSS OPEN   CIRCLE BACK   START EXIT", 48, 502, 2, 151, 168, 179);
  SDL_RenderPresent(r);
}

static void draw_diag(SDL_Renderer *r, const Diag *d) {
  draw_header(r);
  int y = 190, c1 = 170, c2 = 182, c3 = 190;
  if (d->have_pad) {
    textf(r, 48, y, 2, 238, 246, 245, "BUTTONS %08X", (unsigned)d->pad.buttons);
    y += 26;
    textf(r, 48, y, 2, c1, c2, c3, "LEFT STICK %03d %03d   RIGHT STICK %03d %03d", d->pad.lx, d->pad.ly, d->pad.rx, d->pad.ry);
  } else {
    letters(r, "PAD READ FAILED", 48, y, 2, 255, 120, 120);
    y += 26;
  }
  y += 34;
  if (d->have_front) {
    textf(r, 48, y, 2, 238, 246, 245, "FRONT TOUCH %d", (int)d->front.reportNum);
    if (d->front.reportNum > 0) /* coordonnees brutes 1920x1088 -> ecran */
      textf(r, 400, y, 2, c1, c2, c3, "X %04d Y %04d", d->front.report[0].x / 2, d->front.report[0].y / 2);
  } else letters(r, "FRONT TOUCH READ FAILED", 48, y, 2, 255, 120, 120);
  y += 26;
  if (d->have_back) textf(r, 48, y, 2, 238, 246, 245, "BACK TOUCH %d", (int)d->back.reportNum);
  else letters(r, "BACK TOUCH READ FAILED", 48, y, 2, 255, 120, 120);
  y += 34;
  if (d->have_mem) {
    textf(r, 48, y, 2, 238, 246, 245, "FREE USER RAM KB %d", (int)(d->mem.size_user / 1024));
    y += 26;
    textf(r, 48, y, 2, 238, 246, 245, "FREE CDRAM KB %d", (int)(d->mem.size_cdram / 1024));
    y += 26;
    textf(r, 48, y, 2, 238, 246, 245, "FREE PHYCONT KB %d", (int)(d->mem.size_phycont / 1024));
  } else letters(r, "MEMORY QUERY FAILED", 48, y, 2, 255, 120, 120);
  y += 34;
  textf(r, 48, y, 2, c1, c2, c3, "FRAME MS %d   LOG %s", (int)d->frame_ms, d->log_ok ? "OK" : "FAILED");
  for (int i = 0; d->have_front && i < (int)d->front.reportNum && i < 6; i++)
    fill(r, d->front.report[i].x / 2 - 6, d->front.report[i].y / 2 - 6, 12, 12, 46, 197, 169);
  fill(r, 48, 483, 864, 2, 65, 87, 95);
  letters(r, "CIRCLE BACK   START EXIT", 48, 502, 2, 151, 168, 179);
  SDL_RenderPresent(r);
}

int main(int argc, char **argv) {
  (void)argc;
  (void)argv;
  sceIoMkdir("ux0:data", 0777);
  sceIoMkdir(DATA_DIR, 0777);
  g_log = sceIoOpen(LOG_PATH, SCE_O_WRONLY | SCE_O_CREAT | SCE_O_TRUNC, 0666);
  logf_("Horizon Vita bootstrap %s - no Horizon engine in this build", VERSION);

  if (SDL_Init(SDL_INIT_VIDEO) != 0) {
    logf_("SDL_Init failed: %s", SDL_GetError());
    if (g_log >= 0) sceIoClose(g_log);
    return 1;
  }
  SDL_Window *win = SDL_CreateWindow("Horizon Vita DEV", SDL_WINDOWPOS_UNDEFINED, SDL_WINDOWPOS_UNDEFINED, W, H, SDL_WINDOW_SHOWN);
  SDL_Renderer *renderer = win ? SDL_CreateRenderer(win, -1, 0) : NULL;
  if (!renderer) {
    logf_("window/renderer failed: %s", SDL_GetError());
    if (win) SDL_DestroyWindow(win);
    SDL_Quit();
    if (g_log >= 0) sceIoClose(g_log);
    return 2;
  }
  SDL_RendererInfo info;
  if (SDL_GetRendererInfo(renderer, &info) == 0) logf_("SDL renderer: %s", info.name);

  sceCtrlSetSamplingMode(SCE_CTRL_MODE_ANALOG);
  sceTouchSetSamplingState(SCE_TOUCH_PORT_FRONT, SCE_TOUCH_SAMPLING_STATE_START);
  sceTouchSetSamplingState(SCE_TOUCH_PORT_BACK, SCE_TOUCH_SAMPLING_STATE_START);

  Diag d;
  memset(&d, 0, sizeof d);
  d.log_ok = g_log >= 0;
  d.mem.size = sizeof d.mem;
  if (sceKernelGetFreeMemorySize(&d.mem) >= 0)
    logf_("free at start: user %d KiB, cdram %d KiB, phycont %d KiB", (int)(d.mem.size_user / 1024), (int)(d.mem.size_cdram / 1024),
          (int)(d.mem.size_phycont / 1024));

  int selected = 0, page = 0, running = 1;
  uint32_t previous = 0xFFFFFFFFu; /* ignore les boutons deja enfonces au lancement */
  uint32_t last = SDL_GetTicks();
  while (running) {
    SDL_Event ev;
    while (SDL_PollEvent(&ev))
      if (ev.type == SDL_QUIT) running = 0;

    memset(&d.pad, 0, sizeof d.pad);
    d.have_pad = sceCtrlPeekBufferPositive(0, &d.pad, 1) > 0;
    if (d.have_pad) {
      uint32_t pressed = d.pad.buttons & ~previous;
      previous = d.pad.buttons;
      if (pressed & SCE_CTRL_START) running = 0;
      if (pressed & SCE_CTRL_CIRCLE) page = 0;
      if (page != 3) {
        if (pressed & SCE_CTRL_UP) selected = (selected + 3) % 4, page = 0;
        if (pressed & SCE_CTRL_DOWN) selected = (selected + 1) % 4, page = 0;
        if (pressed & SCE_CTRL_CROSS) {
          if (selected == 3) running = 0;
          else page = selected + 1;
          logf_("open item %d", selected);
        }
      }
    }
    if (page == 3) {
      d.have_front = sceTouchPeek(SCE_TOUCH_PORT_FRONT, &d.front, 1) >= 0;
      d.have_back = sceTouchPeek(SCE_TOUCH_PORT_BACK, &d.back, 1) >= 0;
      d.mem.size = sizeof d.mem;
      d.have_mem = sceKernelGetFreeMemorySize(&d.mem) >= 0;
      draw_diag(renderer, &d);
    } else {
      draw_menu(renderer, selected, page);
    }
    SDL_Delay(16);
    uint32_t now = SDL_GetTicks();
    d.frame_ms = now - last;
    last = now;
  }
  logf_("clean exit");
  SDL_DestroyRenderer(renderer);
  SDL_DestroyWindow(win);
  SDL_Quit();
  if (g_log >= 0) sceIoClose(g_log);
  return 0;
}
