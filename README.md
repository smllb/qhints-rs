# qhints-rs

Keyboard-driven UI navigation tool for Linux — Rust rewrite of [qhints](https://github.com/smllb/qhints) (a fork of [hints](https://github.com/AlfredoSequeida/hints) by Alfredo Sequeida). Shows labelled overlays on screen elements, letting you click/hover them via keyboard.

## Status

**Tested on X11 only.** Wayland is not yet supported (see below).

## Requirements

- **X11** session
- **AT-SPI** D-Bus service running (`at-spi-dbus-bus.service`)
- **xdotool** for mouse emulation
- A compositor / window manager — not strictly required, but the overlay rendering relies on features that may not work correctly in all environments

## Build

```sh
cargo build --release
```

The binary is at `target/release/qhints-rs`.

## Configuration

Config file: `~/.config/hints/config.json` (JSON). All fields are optional — defaults are used for missing keys.

See `src/config.rs` for the full list of available settings.

### Example

```json
{
  "alphabet": "asdfgqwertzxcvbhjklyuiopnm",
  "overlay_x_offset": 0,
  "overlay_y_offset": 0,
  "hints": {
    "hint_font_size": 14,
    "hint_background_r": 1.0,
    "hint_background_g": 0.95,
    "hint_background_b": 0.55,
    "hint_background_a": 0.95
  }
}
```

## Usage

Run directly:

```sh
./target/release/qhints-rs
```

Or via the wrapper script (logs to syslog):

```sh
./scripts/run-qhints.sh
```

### i3 keybinding example

```conf
bindsym ctrl+shift+p exec --no-startup-id /home/yogi/qhints-rs/scripts/run-qhints.sh
```

### Controls

| Input | Action |
|-------|--------|
| Type hint keys | Filter/match labels; final key triggers click |
| Ctrl + final hint key | Hover (mousemove only, no click) |
| Alt + final hint key | *Not yet implemented* |
| Escape | Dismiss overlay |

### Options

| Flag | Description |
|------|-------------|
| `-m`, `--mode` | `hint` (default) or `scroll` |
| `-v` | Verbosity (`-v` = debug, `-vv` = trace) |

## Backends

1. **AT-SPI** (primary) — walks the accessibility tree via D-Bus. Fast, async, respects application roles and states.
2. **Imageproc** (fallback) — OCR-based detection using Canny edge detection + contour finding. Used when AT-SPI returns no children.

## Wayland

qhints-rs currently depends on:

- **x11rb** — window geometry, focus tracking, input emulation
- **xdotool** — mouse click simulation
- **GTK3 overlay** — may work under XWayland but is untested

To support Wayland natively, the following would need to be added:

- A `window_system/wayland.rs` backend using a Wayland client library for window info
- `ext-image-capture-src` or similar protocol for screenshot capture
- `libei` / `ydotool` for input emulation
- Testing across compositors (GNOME/KDE/Sway/Hyprland)

Contributions welcome.
