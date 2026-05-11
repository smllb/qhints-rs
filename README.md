# Quantum Hints
![screenshot](https://i.imgur.com/Wibnmd4.jpeg)
Keyboard-driven UI navigation tool for Linux — Rust rewrite of [qhints](https://github.com/smllb/qhints) (a fork of [hints](https://github.com/AlfredoSequeida/hints) by Alfredo Sequeida). Shows labelled overlays on screen elements, letting you click/hover them via keyboard.

**Documentation:** https://smllb.github.io/qhints-rs/

Demonstration on https://youtu.be/BWC7h5dmkI4

## Contents

- [Status](#status)
- [Requirements](#requirements)
- [Build](#build)
- [Usage](#usage)
- [Configuration](#configuration)
  - [Top-level fields](#top-level-fields)
  - [`first_key_zones`](#first_key_zones-default)
  - [`hints` fields](#hints-fields)
  - [`dev` fields](#dev-fields)
  - [`application_rules` fields](#application_rules-fields)
- [Modes](#modes)
- [Backends](#backends)
- [Wayland](#wayland)


## Status

**Tested on X11 only.** Wayland is not yet supported (see below).

## Requirements

- X11 session
- AT-SPI D-Bus service (`at-spi-dbus-bus.service`)
- Rust + Cargo
- xdotool, libgtk-3-dev, librsvg2-dev — for OCR also clang, libclang-dev
- A compositor recommended (tested with picom on i3)

## Build

```sh
cargo build --release
```

For OCR support (optional):

```sh
cargo build --release --features ocr
```

**Note:** The OCR backend downloads ~35MB of models (`text-detection.rten`, `text-recognition.rten`) from AWS S3 on first run to `~/.cache/qhints/ocrs/` ([code](src/backend/ocrs.rs:9-11)).

The binary is at `target/release/qhints-rs`.

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

### Modes

| Key | Behavior | How to use | Advanced |
|-----|----------|------------|----------|
| Type hint keys | Click an element | Type its label | — |
| Ctrl on last key | Hover instead of click | Type the label, hold Ctrl on the last key | — |
| **Alt** (toggle) | Double-click next pick | Press Alt → type a label | — |
| **/** (toggle) | Select text | Press `/` → pick start → pick end | After picking start, press `/` or **Ctrl** → pick end → arrow keys to adjust → **Tab** to switch sides → **Enter** to confirm |
| **Shift** (toggle) | Drag something | Press Shift → pick source → pick destination | After picking source, press **Ctrl** → pick target → arrow keys to adjust → **Tab** to switch sides → **Enter** to confirm. Or press **Shift** again to see all monitors and pick a target anywhere. |
| **Escape** | Dismiss overlay | Press Escape | — |

### Options

| Flag | Description |
|------|-------------|
| `-m`, `--mode` | `hint` (default) or `scroll` |
| `-v` | Verbosity (`-v` = debug, `-vv` = trace) |

## Configuration

Config file: `~/.config/qhints/config.json` (or `$XDG_CONFIG_HOME/qhints/config.json`). All fields are optional — defaults are used for missing keys.

### Top-level fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `complementary_keys_alphabet` | string | `qwertyuiopasdfghjklzxcvbnm` | Characters used for hint labels (second+ chars in multi-char hints) |
| `exit_key` | integer | `65307` (Escape) | Keycode to dismiss the overlay |
| `hover_modifier` | integer | `4` (Ctrl) | Modifier mask held with the final hint key to hover instead of click |
| `double_click_key` | integer | `65513` (Alt) | Toggle double-click mode |
| `text_select_key` | integer | `47` (`/`) | Toggle text selection mode |
| `drag_key` | integer | `65505` (Shift) | Toggle drag mode |
| `advanced_modifier` | integer | `0` | Global key for advanced mode (e.g. `65507` for Ctrl); `0` = per-mode default |
| `overlay_x_offset` | integer | `0` | Horizontal offset for overlay position |
| `overlay_y_offset` | integer | `0` | Vertical offset for overlay position |
| `backends` | array of strings | `["atspi"]` | Backend(s) to use in order |
| `first_key_zones` | array of arrays of strings | 10/9/7 QWERTY grid | Keys assigned to each screen zone |
| `hints` | object | see below | Hint label appearance |
| `application_rules` | object | `{"default": {...}}` | Per-application overrides keyed by app name |

### `first_key_zones` (default)

Defines the keyboard-to-screen spatial mapping. Each outer array element is a **row** (top to bottom); each inner string is a **cell** (left to right). Rows may have different column counts — shorter rows' last cells span horizontally to fill the remaining screen width.

Default: single-key-per-cell for all 26 letters:

```json
[
  ["q","w","e","r","t","y","u","i","o","p"],
  ["a","s","d","f","g","h","j","k","l"],
  ["z","x","c","v","b","n","m"]
]
```

### Hint appearance fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `hint_height` | float | `20.0` | Label height in pixels |
| `hint_width_padding` | float | `10.0` | Horizontal padding inside label |
| `hint_font_size` | float | `14.0` | Font size |
| `hint_font_face` | string | `monospace` | Font family |
| `hint_font_r/g/b/a` | float | `0.16/0.16/0.16/1.0` | Text color |
| `hint_first_font_r/g/b/a` | float | `0.85/0.1/0.1/1.0` | First-character color |
| `hint_first_font_size_boost` | float | `0.0` | Extra size for the first character |
| `hint_pressed_font_r/g/b/a` | float | `0.45/0.75/0.25/1.0` | Typed character color |
| `hint_background_r/g/b/a` | float | `1.0/0.95/0.55/0.95` | Label background |
| `hint_border_r/g/b/a` | float | `0.78/0.72/0.36/1.0` | Label border |
| `hint_border_width` | float | `1.0` | Border width in pixels |
| `hint_corner_radius` | float | `6.0` | Label corner radius |
| `hint_upercase` | bool | `true` | Uppercase hint labels |
| `hint_overlap_threshold` | float | `60.0` | Hint box overlap culling (0=all, 100=aggressive) |
| `hint_opacity` | float | `1.0` | Global hint opacity multiplier |
| `hint_shadow` | bool | `true` | Enable drop shadow |
| `hint_shadow_r/g/b/a` | float | `0.0/0.0/0.0/0.3` | Shadow color |
| `hint_shadow_offset_x/y` | float | `1.0/1.0` | Shadow offset |

### Text selection fields

| Field | Default | Description |
|-------|---------|-------------|
| `text_select_border_r/g/b/a` | blue | Border in text selection mode |
| `text_select_padding_left/right` | `0.0` | Selection offset (fraction of width) |
| `text_select_advanced_key` | `0` | Per-mode advanced toggle key |
| `text_select_nudge_step_x/y` | `0.03` | Arrow nudge step |
| `text_select_nudge_step_shift_x/y` | `0.15` | Shift+arrow nudge step |
| `text_select_pulse_period_ms` | `1200` | Marker pulse period |
| `marker_pulse_interval_ms` | `83` | Pulse redraw rate |
| `marker_bright_duration_ticks` | `10` | Flash duration on placement |
| `text_selection_show_boxes` | `true` | Show blue bounding boxes |

### Drag fields

| Field | Default | Description |
|-------|---------|-------------|
| `drag_advanced_key` | `0` | Per-mode advanced toggle key |
| `drag_delay_ms` | `50` | Delay before mousedown |
| `drag_fullscreen_default` | `true` | Auto fullscreen re-scan after source |
| `drag_marker_shape` | `"circle"` | `"circle"` or `"square"` |
| `drag_marker_size` | `4.0` | Marker radius |
| `drag_show_boxes` | `true` | Show green bounding boxes |

### `advanced_border_extra_width`

Default: `0.25`. Extra border width when in advanced mode.

### `dev` fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `show_grid` | bool | `false` | Draw zone boundaries |
| `hunt` | bool | `false` | Re-scan after every action |
| `hunt_timeout_ms` | integer | `300` | Delay before re-scan (ms) |
| `spotlight` | bool | `false` | Dark overlay with holes around matching hints |
| `spotlight_opacity` | float | `0.65` | Darkness of spotlight |
| `spotlight_radius` | float | `2.5` | Radius multiplier for spotlight holes |
| `advanced_spotlight_opacity` | float | `0.4` | Spotlight in advanced mode |
| `drag_spotlight_opacity` | float | `0.4` | Spotlight in drag mode |
| `show_text_boxes` | bool | `false` | Debug: text word bounding boxes |
| `show_bfs_boxes` | bool | `false` | Debug: BFS component boxes |
| `save_debug_images` | bool | `false` | Save debug PNGs |

### `application_rules` fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `scale_factor` | float | `1.0` | Coordinate scale for HiDPI |
| `detection_scale` | float | `1.0` | Upscale before CV (1–4) |
| `states` | array | `[24, 25, 30]` | AT-SPI states to filter |
| `states_match_type` | int | `1` (all) | Match type |
| `roles` | array | excluded roles | AT-SPI roles to filter |
| `roles_match_type` | int | `3` (none) | Match type |
| `canny_min_val` | int | `15` | Canny edge min threshold |
| `canny_max_val` | int | `40` | Canny edge max threshold |
| `kernel_size` | int | `3` | Canny kernel size |

## Backends

1. **AT-SPI** (primary) — walks the accessibility tree via D-Bus. Fast, async, respects application roles and states. Needs `at-spi-dbus-bus.service` running.
2. **ocrs** (optional, feature-gated) — OCR text detection. Produces word-level hints + BFS gap-filling for icons.
3. **Imageproc** (fallback) — Canny edge detection + BFS connected components + text line projection.

All configured backends run in order and their results are merged. Overlap culling prefers `Text` children over `Element` when overlap exceeds 80%.

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



If this project helps you, consider sponsoring: https://github.com/sponsors/smllb

Contributions welcome.
