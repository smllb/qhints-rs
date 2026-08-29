# Configuration

Config file at `~/.config/qhints/config.json` (or `$XDG_CONFIG_HOME/qhints/config.json`).

All fields are optional. If a field is missing, the Rust default is used.

## Quick example

```json
{
  "backends": ["imageproc"],
  "first_key_zones": [
    ["q","w","e","r","t","y","u","i","o","p"],
    ["a","s","d","f","g","h","n","m","l"]
  ],
  "hints": {
    "hint_opacity": 0.85,
    "hint_font_size": 12
  },
  "dev": {
    "spotlight": true
  }
}
```

## General settings

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `exit_key` | integer | `65307` (Escape) | Keycode to dismiss the overlay |
| `hover_modifier` | integer | `4` (Ctrl) | Modifier held on last key for hover |
| `double_click_key` | integer | `65513` (Alt_L) | Toggle double-click mode |
| `text_select_key` | integer | `47` (`/`) | Toggle text selection mode |
| `drag_key` | integer | `65505` (Shift_L) | Toggle drag mode |
| `advanced_modifier` | integer | `0` | Alternative key for advanced mode (0 = disabled, uses per-mode defaults) |
| `overlay_x_offset` | integer | `0` | Horizontal offset for overlay position |
| `overlay_y_offset` | integer | `0` | Vertical offset for overlay position |
| `backends` | array of strings | `["imageproc"]` | Scanning backends in priority order |
| `hunt` | boolean | `false` | Re-scan after every action |
| `center_zone_padding` | float or object | `0.2` | Fraction of screen excluded from center zone (uniform or `{top,right,bottom,left}`) |

## Hint labels (`first_key_zones`)

Controls which keys occupy which screen zones. A ragged-row grid where each
cell holds the first-character keys for that zone. Example:

```json
[
  ["q","w","e","r","t","y","u","i","o","p"],
  ["a","s","d","f","g","h","j","k","l"],
  ["z","x","c","v","b","n","m"]
]
```

Rows are equal-height bands. Columns within a row are equal-width. Short rows'
last cell spans to fill the remaining width.

## Characters (`complementary_keys_alphabet`)

```json
{ "complementary_keys_alphabet": "qwertyuiopasdfghjklzxcvbnm" }
```

Characters used for the 2nd (and 3rd) characters in multi-char hint labels.
All first-key zone characters must appear in this alphabet.

## Hint appearance

All color fields support separate `_r _g _b _a` values (0.0–1.0) or hex strings:

```json
{ "hints": { "hint_font": "#2a2a2a" } }
```

Hex formats: `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA`.

### Label box

| Field | Default | Description |
|-------|---------|-------------|
| `hint_height` | `20.0` | Label height in pixels |
| `hint_width_padding` | `10.0` | Horizontal padding inside label |
| `hint_corner_radius` | `6.0` | Rounded corner radius |
| `hint_border_width` | `1.0` | Border line width |
| `hint_opacity` | `1.0` | Global alpha multiplier for all visuals |
| `hint_upercase` | `true` | Display labels in uppercase |

### Background

| Field | Default | Description |
|-------|---------|-------------|
| `hint_background_r` | `1.0` | Background red |
| `hint_background_g` | `0.95` | Background green |
| `hint_background_b` | `0.55` | Background blue |
| `hint_background_a` | `0.95` | Background alpha |

### Border

| Field | Default | Description |
|-------|---------|-------------|
| `hint_border_r` | `0.78` | Border red |
| `hint_border_g` | `0.72` | Border green |
| `hint_border_b` | `0.36` | Border blue |
| `hint_border_a` | `1.0` | Border alpha |

### Text

| Field | Default | Description |
|-------|---------|-------------|
| `hint_font_size` | `14.0` | Font size |
| `hint_font_face` | `"monospace"` | Font family |
| `hint_font_r` | `0.16` | Character color red |
| `hint_font_g` | `0.16` | Character color green |
| `hint_font_b` | `0.16` | Character color blue |
| `hint_font_a` | `1.0` | Character alpha |

### First character

| Field | Default | Description |
|-------|---------|-------------|
| `hint_first_font_r` | `0.85` | First-char color red |
| `hint_first_font_g` | `0.1` | First-char color green |
| `hint_first_font_b` | `0.1` | First-char color blue |
| `hint_first_font_a` | `1.0` | First-char alpha |
| `hint_first_font_size_boost` | `0.0` | Extra font size for first char |

### Pressed (typed) state

| Field | Default | Description |
|-------|---------|-------------|
| `hint_pressed_font_r` | `0.45` | Typed-char color red |
| `hint_pressed_font_g` | `0.75` | Typed-char color green |
| `hint_pressed_font_b` | `0.25` | Typed-char color blue |
| `hint_pressed_font_a` | `1.0` | Typed-char alpha |

### Shadow

| Field | Default | Description |
|-------|---------|-------------|
| `hint_shadow` | `true` | Enable drop shadow |
| `hint_shadow_r` | `0.0` | Shadow color red |
| `hint_shadow_g` | `0.0` | Shadow color green |
| `hint_shadow_b` | `0.0` | Shadow color blue |
| `hint_shadow_a` | `0.3` | Shadow opacity |
| `hint_shadow_offset_x` | `1.0` | Shadow x offset |
| `hint_shadow_offset_y` | `1.0` | Shadow y offset |

## Text selection mode settings

| Field | Default | Description |
|-------|---------|-------------|
| `text_select_border_r` | `0.0` | Border red in text selection mode |
| `text_select_border_g` | `0.6` | Border green |
| `text_select_border_b` | `1.0` | Border blue |
| `text_select_border_a` | `1.0` | Border alpha |
| `text_select_padding_left` | `0.0` | Left selection offset (fraction of element width) |
| `text_select_padding_right` | `0.0` | Right selection offset |
| `text_select_advanced_key` | `0` | Per-mode key to toggle advanced mode |
| `text_select_nudge_step_x` | `0.03` | Arrow nudge step X (fraction of element) |
| `text_select_nudge_step_y` | `0.03` | Arrow nudge step Y |
| `text_select_nudge_step_shift_x` | `0.15` | Shift+arrow nudge step X |
| `text_select_nudge_step_shift_y` | `0.15` | Shift+arrow nudge step Y |
| `text_select_pulse_period_ms` | `1200` | Marker pulse animation period |
| `marker_pulse_interval_ms` | `83` | Pulse redraw rate (~60 fps) |
| `marker_bright_duration_ticks` | `10` | Bright flash duration on marker placement |
| `text_selection_show_boxes` | `true` | Show blue bounding boxes around all children |

## Drag mode settings

| Field | Default | Description |
|-------|---------|-------------|
| `drag_advanced_key` | `0` | Per-mode key to toggle advanced drag |
| `drag_delay_ms` | `50` | Delay before mousedown/mouseup |
| `drag_fullscreen_default` | `true` | Auto-trigger fullscreen re-scan after source pick |
| `drag_marker_shape` | `"circle"` | Marker shape (`"circle"` or `"square"`) |
| `drag_marker_size` | `4.0` | Marker radius/half-size |
| `drag_show_boxes` | `true` | Show green bounding boxes around all children |

## Advanced mode

| Field | Default | Description |
|-------|---------|-------------|
| `advanced_border_extra_width` | `0.25` | Extra border width in advanced mode |

## Overlap culling

| Field | Default | Description |
|-------|---------|-------------|
| `hint_overlap_threshold` | `60.0` | 0 = show all, 100 = very aggressive culling |

## Dev options

```json
{
  "dev": {
    "show_grid": false,
    "hunt": false,
    "hunt_timeout_ms": 300,
    "spotlight": false,
    "spotlight_opacity": 0.65,
    "spotlight_radius": 2.5,
    "advanced_spotlight_opacity": 0.4,
    "drag_spotlight_opacity": 0.4,
    "show_text_boxes": false,
    "show_bfs_boxes": false,
    "save_debug_images": false
  }
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `show_grid` | `false` | Draw zone grid boundaries |
| `hunt` | `false` | Re-scan after every action |
| `hunt_timeout_ms` | `300` | Delay before re-scan |
| `spotlight` | `false` | Dark overlay with holes around matching hints |
| `spotlight_opacity` | `0.65` | Darkness of spotlight overlay |
| `spotlight_radius` | `2.5` | Multiplier for spotlight hole radius |
| `advanced_spotlight_opacity` | `0.4` | Spotlight darkness in advanced selection mode |
| `drag_spotlight_opacity` | `0.4` | Spotlight darkness in drag mode |
| `show_text_boxes` | `false` | Blue bounding boxes around text words |
| `show_bfs_boxes` | `false` | Red bounding boxes around BFS components |
| `save_debug_images` | `false` | Save intermediate debug PNGs to `/tmp/qhints_debug/` |

## Application rules

Per-application overrides keyed by the application's WM_CLASS name.

```json
{
  "application_rules": {
    "firefox": {
      "canny_min_val": 20,
      "canny_max_val": 50,
      "kernel_size": 3
    },
    "default": {
      "canny_min_val": 15,
      "canny_max_val": 40,
      "kernel_size": 3
    }
  }
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `scale_factor` | `1.0` | HiDPI coordinate scale |
| `detection_scale` | `1.0` | Screenshot upscale before CV (1–4) |
| `states` | `[24, 25, 30]` | AT-SPI state filter (Sensitive, Showing, Visible) |
| `states_match_type` | `1` (ALL) | Match type: `1`=all, `3`=none |
| `roles` | excluded roles | AT-SPI role filter |
| `roles_match_type` | `3` (NONE) | Match type: `1`=all, `3`=none |
| `canny_min_val` | `15` | Canny low threshold |
| `canny_max_val` | `40` | Canny high threshold |
| `kernel_size` | `3` | Dilation kernel size |
| `text_height_min` | `6` | Min text height (px) for text-height estimate |
| `text_height_max` | `60` | Max text height (px) for text-height estimate |
| `center_zone_padding` | `0.2` | Per-rule center zone padding |
