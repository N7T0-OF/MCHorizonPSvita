#include "vpad_bridge.h"
#include <vpad/input.h>

_Static_assert(sizeof(HorizonVpadSample) == 20, "unexpected VPAD bridge layout");

int32_t
horizon_wiiu_vpad_read(HorizonVpadSample *sample)
{
   VPADStatus status;
   VPADReadError error = VPAD_READ_SUCCESS;
   int32_t count = VPADRead(VPAD_CHAN_0, &status, 1, &error);
   if (error != VPAD_READ_SUCCESS) {
      return (int32_t)error;
   }
   if (count <= 0) {
      return VPAD_READ_NO_SAMPLES;
   }
   sample->buttons = status.hold;
   sample->left_x = status.leftStick.x;
   sample->left_y = status.leftStick.y;
   sample->right_x = status.rightStick.x;
   sample->right_y = status.rightStick.y;
   return 1;
}
