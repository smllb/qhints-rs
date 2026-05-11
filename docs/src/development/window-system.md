# src/window_system/

## src/window_system/mod.rs

```rust
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub extents: (i32, i32, i32, i32),
    pub pid: u32,
    pub app_name: String,
}
```

```rust
pub trait WindowSystem {
    fn focused_window(&self) -> &WindowInfo;
}
```

## src/window_system/x11.rs

```rust
pub struct X11 {
    info: WindowInfo,
}

impl X11 {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>>
}

impl WindowSystem for X11 {
    fn focused_window(&self) -> &WindowInfo
}
```

### `X11::new()` sequence

1. `RustConnection::connect(None)` — open X11 display
2. Intern atoms: `_NET_ACTIVE_WINDOW`, `_NET_WM_PID`, `WM_CLASS`
3. `get_property(_NET_ACTIVE_WINDOW, root)` → active window ID
4. `get_geometry(active_win)` → window size
5. `translate_coordinates(active_win, root, 0, 0)` → absolute position
6. `get_property(_NET_WM_PID)` → PID
7. `get_property(WM_CLASS)` → first null-terminated string as app_name

```rust
pub fn screen_size() -> Result<(i32, i32), Box<dyn std::error::Error>>
```

Connects to X11, returns `(width_in_pixels, height_in_pixels)` of root window.
