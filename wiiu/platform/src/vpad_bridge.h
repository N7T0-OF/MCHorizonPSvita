#pragma once
#include <stdint.h>

/* The caller retains its last sample when no new VPAD sample is available. */
typedef struct HorizonVpadSample {
   uint32_t buttons;
   float left_x, left_y, right_x, right_y;
} HorizonVpadSample;

/* Returns 1 for a sample, VPAD_READ_NO_SAMPLES for no sample, or a VPAD error. */
int32_t horizon_wiiu_vpad_read(HorizonVpadSample *sample);
