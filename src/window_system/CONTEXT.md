# Window System

Abstraction over the display server to get focused window info.

## mod.rs
- `WindowInfo`: extents (x, y, width, height), app name, PID
- `WindowSystem` trait

## x11.rs
- X11 implementation: queries active window via `_NET_ACTIVE_WINDOW`, gets geometry and PID
