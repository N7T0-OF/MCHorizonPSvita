#!/usr/bin/env python3
"""Optional local screenshot/keyboard bridge for an authorized developer session.

Screenshots use a UVC camera feed exposed by VitaSync/udcd_uvc.
Keyboard input is sent to the visible VitaSync PC window, not directly to the
PS Vita kernel. Nothing works without a configured VitaSync input plugin.
"""
import argparse
from pathlib import Path
import sys

DEFAULT_FOLDER = Path(__file__).resolve().parents[1] / 'artifacts' / 'local-only'


def capture(camera_index: int, output: Path) -> int:
    try:
        import cv2
    except ImportError:
        print('Install: python -m pip install opencv-python', file=sys.stderr)
        return 2
    cam = cv2.VideoCapture(camera_index, cv2.CAP_DSHOW if sys.platform == 'win32' else cv2.CAP_ANY)
    if not cam.isOpened():
        print('No UVC video device at the selected camera index.', file=sys.stderr)
        return 3
    try:
        ok, frame = cam.read()
    finally:
        cam.release()
    if not ok:
        print('UVC opened but yielded no frame.', file=sys.stderr)
        return 4
    output.parent.mkdir(parents=True, exist_ok=True)
    if not cv2.imwrite(str(output), frame):
        print('Failed to save screenshot.', file=sys.stderr)
        return 5
    print(f'Screenshot: {output}')
    return 0


def key_press(key: str, title: str, authorized: bool) -> int:
    if not authorized:
        print('Control is disabled. Use --allow-control after approving the session.', file=sys.stderr)
        return 6
    if sys.platform != 'win32':
        print('VitaSync keyboard mode is only supported by this helper on Windows.', file=sys.stderr)
        return 7
    try:
        import pyautogui
        import pygetwindow
    except ImportError:
        print('Install: python -m pip install pyautogui pygetwindow', file=sys.stderr)
        return 8
    windows = [w for w in pygetwindow.getWindowsWithTitle(title) if w.title and w.visible]
    if len(windows) != 1:
        print(f'Need exactly one visible window matching {title!r}; found {len(windows)}.', file=sys.stderr)
        return 9
    # Never silently send input to a random foreground window.
    windows[0].activate()
    pyautogui.press(key)
    print(f'Sent {key!r} to {windows[0].title}; check VitaSync mapping manually.')
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description='Local PS Vita UVC screenshot / optional VitaSync keystrokes')
    sub = parser.add_subparsers(dest='action', required=True)
    c = sub.add_parser('capture', help='capture one frame from UVC webcam')
    c.add_argument('--camera', type=int, default=0, help='UVC camera index (verify with Windows Camera/OBS)')
    c.add_argument('--output', type=Path, default=DEFAULT_FOLDER / 'vita-latest.png')
    k = sub.add_parser('key', help='press a mapped key in an already open VitaSync PC window')
    k.add_argument('name', choices=['up', 'down', 'left', 'right', 'enter', 'esc', 'space', 'w', 'a', 's', 'd'])
    k.add_argument('--window-title', default='VitaSync')
    k.add_argument('--allow-control', action='store_true', help='explicitly authorize input for this call')
    args = parser.parse_args()
    if args.action == 'capture':
        return capture(args.camera, args.output)
    return key_press(args.name, args.window_title, args.allow_control)

if __name__ == '__main__':
    raise SystemExit(main())
