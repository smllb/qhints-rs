use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Path to user config file.
fn config_path() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
            PathBuf::from(home).join(".config")
        });
    config_dir.join("qhints/config.json")
}

// ── AT-SPI integer constants (stable protocol values) ──────────────────────

// Atspi.StateType
pub const ATSPI_STATE_SENSITIVE: i32 = 24;
pub const ATSPI_STATE_SHOWING: i32 = 25;
pub const ATSPI_STATE_VISIBLE: i32 = 30;

// Atspi.CollectionMatchType
pub const ATSPI_MATCH_ALL: i32 = 1;
pub const ATSPI_MATCH_NONE: i32 = 3;

// Atspi.Role values to exclude (NONE match)
pub const EXCLUDED_ROLES: &[i32] = &[
    39,  // PANEL
    85,  // SECTION
    25,  // HTML_CONTAINER
    23,  // FRAME
    34,  // MENU_BAR
    63,  // TOOL_BAR
    31,  // LIST
    38,  // PAGE_TAB_LIST
    121, // DESCRIPTION_LIST
    49,  // SCROLL_PANE
    55,  // TABLE
    99,  // GROUPING
    116, // STATIC
    83,  // HEADING
    73,  // PARAGRAPH
    123, // DESCRIPTION_VALUE
    110, // LANDMARK
    20,  // FILLER
    122, // DESCRIPTION_TERM
];

/// Parse a hex color string (#RGB, #RGBA, #RRGGBB, #RRGGBBAA) into RGBA floats.
fn hex_to_rgba(hex: &str) -> Option<(f64, f64, f64, f64)> {
    let s = hex.trim_start_matches('#');
    let chars: Vec<u8> = match s.len() {
        3 | 4 => s.chars().map(|c| u8::from_str_radix(&c.to_string(), 16).unwrap_or(0) * 17).collect(),
        6 | 8 => (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..(i + 2).min(s.len())], 16).unwrap_or(0)).collect(),
        _ => return None,
    };
    if chars.len() < 3 { return None; }
    let r = chars[0] as f64 / 255.0;
    let g = chars[1] as f64 / 255.0;
    let b = chars[2] as f64 / 255.0;
    let a = if chars.len() > 3 { chars[3] as f64 / 255.0 } else { 1.0 };
    Some((r, g, b, a))
}

// ── Hint appearance defaults ────────────────────────────────────────────────

/// Default hint configuration values.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
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

impl Default for HintStyle {
    fn default() -> Self {
        Self {
            hint_height: 20.0,
            hint_width_padding: 6.0,
            hint_font_size: 12.0,
            hint_font_face: "monospace".into(),
            hint_font_r: 0.16,
            hint_font_g: 0.16,
            hint_font_b: 0.16,
            hint_font_a: 1.0,
            hint_first_font_r: 0.85,
            hint_first_font_g: 0.1,
            hint_first_font_b: 0.1,
            hint_first_font_a: 1.0,
            hint_first_font_size_boost: 0.0,
            hint_overlap_threshold: 60.0,
            hint_pressed_font_r: 0.45,
            hint_pressed_font_g: 0.75,
            hint_pressed_font_b: 0.25,
            hint_pressed_font_a: 1.0,
            hint_upercase: true,
            hint_background_r: 1.0,
            hint_background_g: 0.95,
            hint_background_b: 0.55,
            hint_background_a: 0.95,
            hint_border_r: 0.78,
            hint_border_g: 0.72,
            hint_border_b: 0.36,
            hint_border_a: 1.0,
            hint_border_width: 1.0,
            hint_corner_radius: 6.0,
            text_select_border_r: 0.0,
            text_select_border_g: 0.6,
            text_select_border_b: 1.0,
            text_select_border_a: 1.0,
            text_select_padding_left: 0.0,
            text_select_padding_right: 0.0,
            text_select_advanced_key: 0,
            drag_advanced_key: 0,
            text_select_nudge_step_x: 0.03,
            text_select_nudge_step_y: 0.2,
            text_select_nudge_step_shift_x: 0.15,
            text_select_nudge_step_shift_y: 1.0,
            drag_fullscreen_default: true,
            drag_delay_ms: 10,
            text_select_pulse_period_ms: 1200,
            marker_pulse_interval_ms: 16,
            marker_bright_duration_ticks: 10,
            advanced_border_extra_width: 1.5,
            drag_marker_shape: "square".into(),
            drag_marker_size: 4.0,
            hint_shadow: true,
            hint_shadow_r: 0.0,
            hint_shadow_g: 0.0,
            hint_shadow_b: 0.0,
            hint_shadow_a: 0.3,
            hint_shadow_offset_x: 1.0,
            hint_shadow_offset_y: 1.0,
            text_selection_show_boxes: true,
            drag_show_boxes: true,
            hint_opacity: 0.85,
        }
    }
}

// ── Zone padding (CSS-like, per-side) ───────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct ZonePadding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl ZonePadding {
    pub fn uniform(pad: f64) -> Self {
        Self { top: pad, right: pad, bottom: pad, left: pad }
    }

    fn clamped(mut self) -> Self {
        self.top = self.top.clamp(0.0, 0.49);
        self.right = self.right.clamp(0.0, 0.49);
        self.bottom = self.bottom.clamp(0.0, 0.49);
        self.left = self.left.clamp(0.0, 0.49);
        self
    }
}

impl Default for ZonePadding {
    fn default() -> Self {
        Self::uniform(0.2)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ZonePaddingRepr {
    Num(f64),
    Obj {
        #[serde(default)]
        top: Option<f64>,
        #[serde(default)]
        right: Option<f64>,
        #[serde(default)]
        bottom: Option<f64>,
        #[serde(default)]
        left: Option<f64>,
    },
}

impl<'de> Deserialize<'de> for ZonePadding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let base = Self::default();
        match ZonePaddingRepr::deserialize(deserializer)? {
            ZonePaddingRepr::Num(pad) => Ok(Self::uniform(pad).clamped()),
            ZonePaddingRepr::Obj { top, right, bottom, left } => Ok(Self {
                top: top.unwrap_or(base.top),
                right: right.unwrap_or(base.right),
                bottom: bottom.unwrap_or(base.bottom),
                left: left.unwrap_or(base.left),
            }
            .clamped()),
        }
    }
}

// ── Dev options ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
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

impl Default for DevOptions {
    fn default() -> Self {
        Self {
            show_grid: false,
            hunt: false,
            hunt_timeout_ms: 1000,
            spotlight: true,
            spotlight_opacity: 0.10,
            spotlight_radius: 5.0,
            advanced_spotlight_opacity: 0.4,
            drag_spotlight_opacity: 0.4,
            show_text_boxes: false,
            show_bfs_boxes: false,
            save_debug_images: true,
        }
    }
}

// ── Application rules ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
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
    /// Also run Canny on the min-of-RGB channel (ORed with max-of-RGB) to
    /// recover bright colored text (e.g. orange on white) that max-of-RGB is
    /// blind to.
    pub min_channel_edges: bool,
    pub center_zone_padding: ZonePadding,
}

impl Default for ApplicationRule {
    fn default() -> Self {
        Self {
            scale_factor: 1.0,
            detection_scale: 1.0,
            states: vec![ATSPI_STATE_SENSITIVE, ATSPI_STATE_SHOWING, ATSPI_STATE_VISIBLE],
            states_match_type: ATSPI_MATCH_ALL,
            roles: EXCLUDED_ROLES.to_vec(),
            roles_match_type: ATSPI_MATCH_NONE,
            canny_min_val: 15,
            canny_max_val: 40,
            kernel_size: 3,
            min_channel_edges: true,
            center_zone_padding: ZonePadding::uniform(0.2),
        }
    }
}

// ── Top-level config ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
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
    /// Legacy top-level alias for `dev.hunt`.
    pub hunt: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hints: HintStyle::default(),
            complementary_keys_alphabet: "qwertyuiopasdfghjklzxcvbnm".into(),
            exit_key: 65307,   // GDK_KEY_Escape
            hover_modifier: 4, // CONTROL_MASK
            double_click_key: 65513, // GDK_KEY_Alt_L (Alt key)
            advanced_modifier: 65507, // Ctrl
            drag_key: 65505, // GDK_KEY_Shift_L
            text_select_key: 47, // GDK_KEY_slash (/)
            overlay_x_offset: 0,
            overlay_y_offset: 0,
            application_rules: {
                let mut m = HashMap::new();
                m.insert("default".into(), ApplicationRule::default());
                m
            },
            backends: vec!["imageproc".into()],
            first_key_zones: vec![
                vec!["q".into(), "w".into(), "e".into(), "r".into(), "t".into(), "y".into(), "u".into(), "i".into(), "o".into(), "p".into()],
                vec!["a".into(), "s".into(), "d".into(), "f".into(), "g".into(), "h".into(), "j".into(), "k".into(), "l".into()],
                vec!["z".into(), "x".into(), "c".into(), "v".into(), "b".into(), "n".into(), "m".into()],
            ],
            center_zone_padding: ZonePadding::uniform(0.3),
            dev: DevOptions::default(),
            hunt: None,
        }
    }
}

/// Load config, merging user JSON over Rust defaults.
pub fn load_config() -> Config {
    let path = config_path();

    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            return parse_config(&data);
        }
    }

    Config::default()
}

/// Parse config JSON, merging it over Rust defaults.
fn parse_config(data: &str) -> Config {
    let mut config = Config::default();

    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(data) {
        preprocess_colors(&mut json);
        match serde_json::from_value::<Config>(json) {
            Ok(user) => config = user,
            Err(e) => log::warn!("Failed to parse config: {}", e),
        }
    }

    config.normalize();
    config
}

/// Convert hex color strings (e.g. `"hint_font": "#2a2a2a"`) in the user JSON
/// into the matching `_r/_g/_b/_a` float fields before deserialization.
fn preprocess_colors(json: &mut serde_json::Value) {
    let Some(hints) = json.get_mut("hints").and_then(|v| v.as_object_mut()) else {
        return;
    };

    const HEX_COLORS: [(&str, &str, &str, &str, &str); 7] = [
        ("hint_font", "hint_font_r", "hint_font_g", "hint_font_b", "hint_font_a"),
        ("hint_first_font", "hint_first_font_r", "hint_first_font_g", "hint_first_font_b", "hint_first_font_a"),
        ("hint_pressed_font", "hint_pressed_font_r", "hint_pressed_font_g", "hint_pressed_font_b", "hint_pressed_font_a"),
        ("hint_background", "hint_background_r", "hint_background_g", "hint_background_b", "hint_background_a"),
        ("hint_border", "hint_border_r", "hint_border_g", "hint_border_b", "hint_border_a"),
        ("text_select_border", "text_select_border_r", "text_select_border_g", "text_select_border_b", "text_select_border_a"),
        ("hint_shadow", "hint_shadow_r", "hint_shadow_g", "hint_shadow_b", "hint_shadow_a"),
    ];

    for (hex_key, r_key, g_key, b_key, a_key) in HEX_COLORS {
        let Some(hex) = hints.get(hex_key).and_then(|v| v.as_str()).map(str::to_string) else {
            continue;
        };
        let Some((r, g, b, a)) = hex_to_rgba(&hex) else {
            continue;
        };
        hints.insert(r_key.to_string(), serde_json::json!(r));
        hints.insert(g_key.to_string(), serde_json::json!(g));
        hints.insert(b_key.to_string(), serde_json::json!(b));
        hints.insert(a_key.to_string(), serde_json::json!(a));
        hints.remove(hex_key);
    }
}

impl Config {
    /// Clamp/validate values loaded from the user config.
    fn normalize(&mut self) {
        self.hints.hint_opacity = self.hints.hint_opacity.clamp(0.0, 1.0);
        self.hints.advanced_border_extra_width = self.hints.advanced_border_extra_width.max(0.0);
        self.hints.drag_marker_size = self.hints.drag_marker_size.max(0.0);
        self.center_zone_padding = self.center_zone_padding.clamped();

        self.dev.spotlight_opacity = self.dev.spotlight_opacity.clamp(0.0, 1.0);
        self.dev.spotlight_radius = self.dev.spotlight_radius.max(1.0);
        self.dev.advanced_spotlight_opacity = self.dev.advanced_spotlight_opacity.clamp(0.0, 1.0);
        self.dev.drag_spotlight_opacity = self.dev.drag_spotlight_opacity.clamp(0.0, 1.0);

        if let Some(hunt) = self.hunt {
            self.dev.hunt = hunt;
        }

        for rule in self.application_rules.values_mut() {
            rule.detection_scale = rule.detection_scale.clamp(0.1, 4.0);
            rule.center_zone_padding = rule.center_zone_padding.clamped();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_empty_json() {
        let c = parse_config("{}");
        assert_eq!(c.exit_key, 65307);
        assert_eq!(c.backends, vec!["imageproc".to_string()]);
        assert_eq!(c.hints.hint_height, 20.0);
    }

    #[test]
    fn overrides_top_level() {
        let c = parse_config(r#"{ "exit_key": 42, "backends": ["atspi"] }"#);
        assert_eq!(c.exit_key, 42);
        assert_eq!(c.backends, vec!["atspi".to_string()]);
        assert_eq!(c.text_select_key, 47);
    }

    #[test]
    fn hex_color_converts_to_rgba() {
        let c = parse_config(r##"{ "hints": { "hint_font": "#ffffff" } }"##);
        assert_eq!(c.hints.hint_font_r, 1.0);
        assert_eq!(c.hints.hint_font_g, 1.0);
        assert_eq!(c.hints.hint_font_b, 1.0);
        assert_eq!(c.hints.hint_font_a, 1.0);
    }

    #[test]
    fn clamps_out_of_range_values() {
        let c = parse_config(
            r#"{ "hints": { "hint_opacity": 5.0 }, "dev": { "spotlight_radius": 0.1 }, "center_zone_padding": 0.9 }"#,
        );
        assert_eq!(c.hints.hint_opacity, 1.0);
        assert_eq!(c.dev.spotlight_radius, 1.0);
        assert_eq!(c.center_zone_padding.top, 0.49);
    }

    #[test]
    fn zone_padding_accepts_number_or_object() {
        let num = parse_config(r#"{ "center_zone_padding": 0.4 }"#);
        assert_eq!(num.center_zone_padding.top, 0.4);
        assert_eq!(num.center_zone_padding.bottom, 0.4);

        let obj = parse_config(
            r#"{ "center_zone_padding": { "top": 0.1, "right": 0.2, "bottom": 0.3, "left": 0.4 } }"#,
        );
        assert_eq!(obj.center_zone_padding.top, 0.1);
        assert_eq!(obj.center_zone_padding.left, 0.4);
    }

    #[test]
    fn partial_app_rule_keeps_defaults() {
        let c = parse_config(r#"{ "application_rules": { "firefox": { "scale_factor": 2.0 } } }"#);
        let r = &c.application_rules["firefox"];
        assert_eq!(r.scale_factor, 2.0);
        assert_eq!(r.canny_min_val, 15);
        assert_eq!(r.states, vec![24, 25, 30]);
    }

    #[test]
    fn legacy_top_level_hunt_aliases_dev() {
        let c = parse_config(r#"{ "hunt": true }"#);
        assert!(c.dev.hunt);
    }
}
