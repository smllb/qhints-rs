# src/config.rs

## Constants

```rust
pub const ATSPI_STATE_SENSITIVE: i32 = 24;
pub const ATSPI_STATE_SHOWING: i32 = 25;
pub const ATSPI_STATE_VISIBLE: i32 = 30;
pub const ATSPI_MATCH_ALL: i32 = 1;
pub const ATSPI_MATCH_NONE: i32 = 3;
pub const EXCLUDED_ROLES: &[i32] = &[
    39, 85, 25, 23, 34, 63, 31, 38, 121, 49, 55, 99, 116, 83, 73, 123, 110, 20, 122
];
```

## Private helpers

```rust
fn config_path() -> PathBuf
fn hex_to_rgba(hex: &str) -> Option<(f64, f64, f64, f64)>
```

## Structs

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct HintStyle {
    pub hint_height: f64,
    pub hint_width_padding: f64,
    pub hint_font_size: f64,
    pub hint_font_face: String,
    pub hint_font_r: f64,
    pub hint_font_g: f64,
    pub hint_font_b: f64,
    pub hint_font_a: f64,
    pub hint_first_font_r: f64,
    pub hint_first_font_g: f64,
    pub hint_first_font_b: f64,
    pub hint_first_font_a: f64,
    pub hint_first_font_size_boost: f64,
    pub hint_overlap_threshold: f64,
    pub hint_pressed_font_r: f64,
    pub hint_pressed_font_g: f64,
    pub hint_pressed_font_b: f64,
    pub hint_pressed_font_a: f64,
    pub hint_upercase: bool,
    pub hint_background_r: f64,
    pub hint_background_g: f64,
    pub hint_background_b: f64,
    pub hint_background_a: f64,
    pub hint_border_r: f64,
    pub hint_border_g: f64,
    pub hint_border_b: f64,
    pub hint_border_a: f64,
    pub hint_border_width: f64,
    pub hint_corner_radius: f64,
    pub text_select_border_r: f64,
    pub text_select_border_g: f64,
    pub text_select_border_b: f64,
    pub text_select_border_a: f64,
    pub text_select_padding_left: f64,
    pub text_select_padding_right: f64,
    pub text_select_advanced_key: u32,
    pub drag_advanced_key: u32,
    pub text_select_nudge_step_x: f64,
    pub text_select_nudge_step_y: f64,
    pub text_select_nudge_step_shift_x: f64,
    pub text_select_nudge_step_shift_y: f64,
    pub drag_fullscreen_default: bool,
    pub drag_delay_ms: u64,
    pub text_select_pulse_period_ms: u64,
    pub marker_pulse_interval_ms: u64,
    pub marker_bright_duration_ticks: u32,
    pub advanced_border_extra_width: f64,
    pub drag_marker_shape: String,
    pub drag_marker_size: f64,
    pub hint_shadow: bool,
    pub hint_shadow_r: f64,
    pub hint_shadow_g: f64,
    pub hint_shadow_b: f64,
    pub hint_shadow_a: f64,
    pub hint_shadow_offset_x: f64,
    pub hint_shadow_offset_y: f64,
    pub text_selection_show_boxes: bool,
    pub drag_show_boxes: bool,
    pub hint_opacity: f64,
}

impl Default for HintStyle
```

```rust
#[derive(Debug, Clone, Copy)]
pub struct ZonePadding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl ZonePadding {
    pub fn uniform(pad: f64) -> Self
}
```

```rust
#[derive(Debug, Clone)]
pub struct DevOptions {
    pub show_grid: bool,
    pub hunt: bool,
    pub hunt_timeout_ms: u32,
    pub spotlight: bool,
    pub spotlight_opacity: f64,
    pub spotlight_radius: f64,
    pub advanced_spotlight_opacity: f64,
    pub drag_spotlight_opacity: f64,
    pub show_text_boxes: bool,
    pub show_bfs_boxes: bool,
    pub save_debug_images: bool,
}

impl Default for DevOptions
```

```rust
#[derive(Debug, Clone)]
pub struct ApplicationRule {
    pub scale_factor: f64,
    pub detection_scale: f64,
    pub states: Vec<i32>,
    pub states_match_type: i32,
    pub roles: Vec<i32>,
    pub roles_match_type: i32,
    pub canny_min_val: i32,
    pub canny_max_val: i32,
    pub kernel_size: i32,
    pub center_zone_padding: ZonePadding,
}

impl Default for ApplicationRule
```

```rust
#[derive(Debug, Clone)]
pub struct Config {
    pub hints: HintStyle,
    pub complementary_keys_alphabet: String,
    pub exit_key: u32,
    pub hover_modifier: u32,
    pub double_click_key: u32,
    pub advanced_modifier: u32,
    pub drag_key: u32,
    pub text_select_key: u32,
    pub overlay_x_offset: i32,
    pub overlay_y_offset: i32,
    pub application_rules: HashMap<String, ApplicationRule>,
    pub backends: Vec<String>,
    pub first_key_zones: Vec<Vec<String>>,
    pub center_zone_padding: ZonePadding,
    pub dev: DevOptions,
}

impl Default for Config
```

## Public functions

```rust
pub fn load_config() -> Config
```

Loads config from `$XDG_CONFIG_HOME/qhints/config.json`, merging over
`Config::default()`. If the file doesn't exist, returns defaults.

## Private functions

```rust
fn parse_config(data: &str) -> Config
fn preprocess_colors(json: &mut serde_json::Value)
fn hex_to_rgba(hex: &str) -> Option<(f64, f64, f64, f64)>
```

Config is deserialized directly with `serde` (`#[serde(default)]` on every
struct), so user JSON is merged over `Config::default()` automatically.
`preprocess_colors` rewrites hex color strings (e.g. `"hint_font": "#2a2a2a"`)
into `_r/_g/_b/_a` float fields before deserialization. `Config::normalize`
then clamps out-of-range values.
