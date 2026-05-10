# qhints-rs
![screenshot](https://i.imgur.com/Wibnmd4.jpeg)
Keyboard-driven UI navigation tool for Linux — Rust rewrite of [qhints](https://github.com/smllb/qhints) (a fork of [hints](https://github.com/AlfredoSequeida/hints) by Alfredo Sequeida). Shows labelled overlays on screen elements, letting you click/hover them via keyboard.

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

Config file: `~/.config/qhints/config.json` (or `$XDG_CONFIG_HOME/qhints/config.json`). All fields are optional — defaults are used for missing keys.

### Top-level fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `complementary_keys_alphabet` | string | `asdfgqwertzxcvbhjklyuiopnm` | Characters used for hint labels (second+ chars in multi-char hints) |
| `exit_key` | integer | `65307` (Escape) | Keycode to dismiss the overlay |
| `hover_modifier` | integer | `4` (Ctrl) | Modifier mask held with the final hint key to hover instead of click |
| `double_click_key` | integer | `65513` (Alt) | Toggle double-click mode |
| `text_select_key` | integer | `47` (`/`) | Toggle text selection mode |
| `drag_key` | integer | `65505` (Shift) | Toggle drag mode |
| `advanced_modifier` | integer | `0` | Global key for advanced mode (e.g. `65507` for Ctrl); `0` = per-mode default |
| `overlay_x_offset` | integer | `0` | Horizontal offset for overlay position |
| `overlay_y_offset` | integer | `0` | Vertical offset for overlay position |
| `backends` | array of strings | `["atspi"]` | Backend(s) to use in order |
| `first_key_zones` | array of arrays of strings | see below | Keys assigned to each screen zone — grid of rows×cols, first char of each hint |
| `hints` | object | see below | Hint label appearance |
| `application_rules` | object | `{"default": {...}}` | Per-application overrides keyed by app name |

### `first_key_zones` (default)

Defines the keyboard-to-screen spatial mapping.  Each outer array element is a **row** (top to bottom); each inner string is a **cell** (left to right) holding the keys for that screen zone.  Rows may have different column counts — shorter rows' last cells span horizontally to fill the remaining screen width.

Rows are equal-height bands.  Within each row, columns are equal-width.

Each key in the zone *must* appear in `complementary_keys_alphabet`.

```json
[
  ["qwe", "rty", "uiop"],
  ["asd", "fgh", "nml"],
  ["zxc", "vb",  "jk"]
]
```

For a ragged layout (e.g. 10 / 9 / 7 columns):

```json
[
  ["q","w","e","r","t","y","u","i","o","p"],
  ["a","s","d","f","g","h","n","m","l"],
  ["z","x","c","v","b","j","k"]
]
```

### `hints` fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `hint_height` | float | `20.0` | Label height in pixels |
| `hint_width_padding` | float | `10.0` | Horizontal padding inside label |
| `hint_font_size` | float | `14.0` | Font size |
| `hint_font_face` | string | `monospace` | Font family |
| `hint_font_r` | float | `0.16` | Text color (red 0–1) |
| `hint_font_g` | float | `0.16` | Text color (green 0–1) |
| `hint_font_b` | float | `0.16` | Text color (blue 0–1) |
| `hint_font_a` | float | `1.0` | Text opacity |
| `hint_first_font_r` | float | `0.85` | First-character color (red) |
| `hint_first_font_g` | float | `0.1` | First-character color (green) |
| `hint_first_font_b` | float | `0.1` | First-character color (blue) |
| `hint_first_font_a` | float | `1.0` | First-character opacity |
| `hint_first_font_size_boost` | float | `0.0` | Extra size for the first character |
| `hint_overlap_threshold` | float | `60.0` | Pixel distance to consider hints overlapping |
| `hint_pressed_font_r` | float | `0.45` | Pressed state text color (red) |
| `hint_pressed_font_g` | float | `0.75` | Pressed state text color (green) |
| `hint_pressed_font_b` | float | `0.25` | Pressed state text color (blue) |
| `hint_pressed_font_a` | float | `1.0` | Pressed state opacity |
| `hint_upercase` | bool | `true` | Uppercase hint labels |
| `hint_background_r` | float | `1.0` | Background color (red) |
| `hint_background_g` | float | `0.95` | Background color (green) |
| `hint_background_b` | float | `0.55` | Background color (blue) |
| `hint_background_a` | float | `0.95` | Background opacity |
| `hint_border_r` | float | `0.78` | Border color (red) |
| `hint_border_g` | float | `0.72` | Border color (green) |
| `hint_border_b` | float | `0.36` | Border color (blue) |
| `hint_border_a` | float | `1.0` | Border opacity |
| `hint_border_width` | float | `1.0` | Border width in pixels |
| `hint_corner_radius` | float | `6.0` | Label corner radius |
| `hint_shadow` | bool | `true` | Enable drop shadow |
| `hint_shadow_r` | float | `0.0` | Shadow color (red) |
| `hint_shadow_g` | float | `0.0` | Shadow color (green) |
| `hint_shadow_b` | float | `0.0` | Shadow color (blue) |
| `hint_shadow_a` | float | `0.3` | Shadow opacity |
| `hint_shadow_offset_x` | float | `1.0` | Shadow horizontal offset |
| `hint_shadow_offset_y` | float | `1.0` | Shadow vertical offset |
| `text_select_border_r` | float | `0.0` | Text selection border color (red) |
| `text_select_border_g` | float | `0.6` | Text selection border color (green) |
| `text_select_border_b` | float | `1.0` | Text selection border color (blue) |
| `text_select_border_a` | float | `1.0` | Text selection border opacity |
| `text_select_padding_left` | float | `0.0` | Selection start offset (fraction of element width) |
| `text_select_padding_right` | float | `0.0` | Selection end offset (fraction of element width) |
| `text_select_advanced_key` | integer | `0` | Per-mode advanced key for text selection |
| `drag_advanced_key` | integer | `0` | Per-mode advanced key for drag mode |
| `text_select_nudge_step_x` | float | `0.03` | Arrow key nudge step horizontal |
| `text_select_nudge_step_y` | float | `0.03` | Arrow key nudge step vertical |
| `text_select_nudge_step_shift_x` | float | `0.15` | Shift+arrow nudge horizontal |
| `text_select_nudge_step_shift_y` | float | `0.15` | Shift+arrow nudge vertical |
| `text_select_pulse_period_ms` | integer | `1200` | Marker pulse animation period |
| `marker_pulse_interval_ms` | integer | `83` | Pulse redraw rate (lower = smoother) |
| `drag_fullscreen_default` | bool | `true` | Second drag hint triggers fullscreen re-scan |
| `text_selection_show_boxes` | bool | `true` | Show bounding boxes around all children when text selection mode is active |
| `drag_show_boxes` | bool | `true` | Show bounding boxes around all children when drag mode is active |

### `dev` fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `show_grid` | bool | `false` | Draw zone boundaries |
| `hunt` | bool | `false` | Re-scan after every action |
| `hunt_timeout_ms` | integer | `300` | Delay before re-scan (ms) |
| `spotlight` | bool | `false` | Dark overlay with holes around matching hints |
| `spotlight_opacity` | float | `0.65` | Darkness of spotlight |
| `spotlight_radius` | float | `2.5` | Radius multiplier for spotlight holes |
| `advanced_spotlight_opacity` | float | `0.4` | Darkness of advanced mode spotlight |
| `drag_spotlight_opacity` | float | `0.4` | Darkness of drag mode spotlight |

### `application_rules` fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `scale_factor` | float | `1.0` | Scale factor for element coordinates |
| `states` | array of int | `[24, 25, 30]` | AT-SPI states to filter |
| `states_match_type` | int | `1` (all) | Match type: 1=all, 3=none |
| `roles` | array of int | excluded roles | AT-SPI roles to filter |
| `roles_match_type` | int | `3` (none) | Match type: 1=all, 3=none |
| `canny_min_val` | int | `15` | Canny edge detection min threshold |
| `canny_max_val` | int | `40` | Canny edge detection max threshold |
| `kernel_size` | int | `3` | Canny kernel size |

### Example

```json
{
  "complementary_keys_alphabet": "asdfgqwertzxcvbhjklyuiopnm",
  "overlay_x_offset": 0,
  "overlay_y_offset": 0,
  "first_key_zones": [
    ["q","w","e","r","t","y","u","i","o","p"],
    ["a","s","d","f","g","h","n","m","l"],
    ["z","x","c","v","b","j","k"]
  ],
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

Contributions welcome.
