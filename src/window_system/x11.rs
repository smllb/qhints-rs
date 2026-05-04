use super::{WindowInfo, WindowSystem};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

/// X11 window system using x11rb.
pub struct X11 {
    info: WindowInfo,
}

impl X11 {
    /// Connect to X11 display and query the active window.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let (conn, screen_num) = RustConnection::connect(None)?;
        let setup = conn.setup();
        let screen = &setup.roots[screen_num];
        let root = screen.root;

        // Intern atoms
        let net_active = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
            .reply()?
            .atom;
        let net_wm_pid = conn.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
        let wm_class_atom = conn.intern_atom(false, b"WM_CLASS")?.reply()?.atom;

        // Get active window ID
        let active_reply = conn
            .get_property(false, root, net_active, AtomEnum::WINDOW, 0, 1)?
            .reply()?;

        let active_win = active_reply
            .value32()
            .and_then(|mut iter| iter.next())
            .ok_or("No active window found")?;

        if active_win == 0 {
            return Err("Active window is None".into());
        }

        // Get geometry
        let geom = conn.get_geometry(active_win)?.reply()?;
        let translated = conn
            .translate_coordinates(active_win, root, 0, 0)?
            .reply()?;

        let extents = (
            translated.dst_x as i32,
            translated.dst_y as i32,
            geom.width as i32,
            geom.height as i32,
        );

        // Get PID
        let pid_reply = conn
            .get_property(false, active_win, net_wm_pid, AtomEnum::CARDINAL, 0, 1)?
            .reply()?;
        let pid = pid_reply
            .value32()
            .and_then(|mut iter| iter.next())
            .unwrap_or(0);

        // Get WM_CLASS
        let class_reply = conn
            .get_property(false, active_win, wm_class_atom, AtomEnum::STRING, 0, 1024)?
            .reply()?;
        let app_name = if !class_reply.value.is_empty() {
            // WM_CLASS is two null-separated strings: instance\0class\0
            let raw = &class_reply.value;
            let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
            String::from_utf8_lossy(&raw[..end]).into_owned()
        } else {
            String::new()
        };

        Ok(Self {
            info: WindowInfo {
                extents,
                pid,
                app_name,
            },
        })
    }
}

impl WindowSystem for X11 {
    fn focused_window(&self) -> &WindowInfo {
        &self.info
    }
}
