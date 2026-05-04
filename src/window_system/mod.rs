pub mod x11;

/// Window geometry + metadata from the window system.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// Absolute position and size: (x, y, width, height).
    pub extents: (i32, i32, i32, i32),
    /// PID of the focused window's process.
    pub pid: u32,
    /// WM_CLASS instance name (used for per-app rules).
    pub app_name: String,
}

/// Trait for window system backends.
pub trait WindowSystem {
    /// Get info about the currently focused window.
    fn focused_window(&self) -> &WindowInfo;
}
