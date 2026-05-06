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

// ── Hint appearance defaults ────────────────────────────────────────────────

/// Default hint configuration values.
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
    pub hint_shadow: bool,
    pub hint_shadow_r: f64,
    pub hint_shadow_g: f64,
    pub hint_shadow_b: f64,
    pub hint_shadow_a: f64,
    pub hint_shadow_offset_x: f64,
    pub hint_shadow_offset_y: f64,
}

impl Default for HintStyle {
    fn default() -> Self {
        Self {
            hint_height: 20.0,
            hint_width_padding: 10.0,
            hint_font_size: 14.0,
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
            hint_shadow: true,
            hint_shadow_r: 0.0,
            hint_shadow_g: 0.0,
            hint_shadow_b: 0.0,
            hint_shadow_a: 0.3,
            hint_shadow_offset_x: 1.0,
            hint_shadow_offset_y: 1.0,
        }
    }
}

// ── Application rules ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ApplicationRule {
    pub scale_factor: f64,
    pub states: Vec<i32>,
    pub states_match_type: i32,
    pub roles: Vec<i32>,
    pub roles_match_type: i32,
    pub canny_min_val: i32,
    pub canny_max_val: i32,
    pub kernel_size: i32,
}

impl Default for ApplicationRule {
    fn default() -> Self {
        Self {
            scale_factor: 1.0,
            states: vec![ATSPI_STATE_SENSITIVE, ATSPI_STATE_SHOWING, ATSPI_STATE_VISIBLE],
            states_match_type: ATSPI_MATCH_ALL,
            roles: EXCLUDED_ROLES.to_vec(),
            roles_match_type: ATSPI_MATCH_NONE,
            canny_min_val: 30,
            canny_max_val: 70,
            kernel_size: 3,
        }
    }
}

// ── Top-level config ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Config {
    pub hints: HintStyle,
    pub complementary_keys_alphabet: String,
    pub exit_key: u32,
    pub hover_modifier: u32,
    pub grab_modifier: u32,
    pub overlay_x_offset: i32,
    pub overlay_y_offset: i32,
    pub application_rules: HashMap<String, ApplicationRule>,
    pub backends: Vec<String>,
    pub first_key_zones: [[String; 3]; 3],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hints: HintStyle::default(),
            complementary_keys_alphabet: "asdfgqwertzxcvbhjklyuiopnm".into(),
            exit_key: 65307,   // GDK_KEY_Escape
            hover_modifier: 4, // CONTROL_MASK
            grab_modifier: 8,  // MOD1_MASK (Alt)
            overlay_x_offset: 0,
            overlay_y_offset: 0,
            application_rules: {
                let mut m = HashMap::new();
                m.insert("default".into(), ApplicationRule::default());
                m
            },
            backends: vec!["atspi".into()],
            first_key_zones: [
                ["qwe".into(), "rty".into(), "uiop".into()],
                ["asd".into(), "fgh".into(), "nml".into()],
                ["zxc".into(), "vb".into(), "jk".into()],
            ],
        }
    }
}

/// Load config, merging user JSON over defaults.
pub fn load_config() -> Config {
    let mut config = Config::default();
    let path = config_path();

    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(user_json) = serde_json::from_str::<serde_json::Value>(&data) {
                // Merge user overrides into default config
                merge_user_config(&mut config, &user_json);
            }
        }
    }

    config
}

/// Merge user JSON values into the config struct.
fn merge_user_config(config: &mut Config, json: &serde_json::Value) {
    if let Some(complementary_keys_alphabet) = json.get("complementary_keys_alphabet").and_then(|v| v.as_str()) {
        config.complementary_keys_alphabet = complementary_keys_alphabet.into();
    }
    if let Some(x) = json.get("overlay_x_offset").and_then(|v| v.as_i64()) {
        config.overlay_x_offset = x as i32;
    }
    if let Some(y) = json.get("overlay_y_offset").and_then(|v| v.as_i64()) {
        config.overlay_y_offset = y as i32;
    }
    if let Some(k) = json.get("exit_key").and_then(|v| v.as_i64()) {
        config.exit_key = k as u32;
    }
    if let Some(k) = json.get("hover_modifier").and_then(|v| v.as_i64()) {
        config.hover_modifier = k as u32;
    }
    if let Some(k) = json.get("grab_modifier").and_then(|v| v.as_i64()) {
        config.grab_modifier = k as u32;
    }

    if let Some(zones) = json.get("first_key_zones").and_then(|v| v.as_array()) {
        for (r, row) in zones.iter().enumerate() {
            if r >= 3 { break; }
            if let Some(row_arr) = row.as_array() {
                for (c, cell) in row_arr.iter().enumerate() {
                    if c >= 3 { break; }
                    if let Some(s) = cell.as_str() {
                        config.first_key_zones[r][c] = s.into();
                    }
                }
            }
        }
    }

    if let Some(backends) = json.get("backends").and_then(|v| v.as_array()) {
        config.backends = backends
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }

    if let Some(rules) = json.get("application_rules").and_then(|v| v.as_object()) {
        for (app_name, rule_val) in rules {
            if let Some(rule_obj) = rule_val.as_object() {
                let mut rule = config.application_rules
                    .get(app_name)
                    .cloned()
                    .unwrap_or_default();
                if let Some(v) = rule_obj.get("scale_factor").and_then(|v| v.as_f64()) {
                    rule.scale_factor = v;
                }
                if let Some(v) = rule_obj.get("states_match_type").and_then(|v| v.as_i64()) {
                    rule.states_match_type = v as i32;
                }
                if let Some(v) = rule_obj.get("roles_match_type").and_then(|v| v.as_i64()) {
                    rule.roles_match_type = v as i32;
                }
                if let Some(v) = rule_obj.get("canny_min_val").and_then(|v| v.as_i64()) {
                    rule.canny_min_val = v as i32;
                }
                if let Some(v) = rule_obj.get("canny_max_val").and_then(|v| v.as_i64()) {
                    rule.canny_max_val = v as i32;
                }
                if let Some(v) = rule_obj.get("kernel_size").and_then(|v| v.as_i64()) {
                    rule.kernel_size = v as i32;
                }
                if let Some(arr) = rule_obj.get("states").and_then(|v| v.as_array()) {
                    rule.states = arr.iter().filter_map(|v| v.as_i64().map(|i| i as i32)).collect();
                }
                if let Some(arr) = rule_obj.get("roles").and_then(|v| v.as_array()) {
                    rule.roles = arr.iter().filter_map(|v| v.as_i64().map(|i| i as i32)).collect();
                }
                config.application_rules.insert(app_name.clone(), rule);
            }
        }
    }

    // Merge hint style overrides
    if let Some(hints) = json.get("hints").and_then(|v| v.as_object()) {
        let h = &mut config.hints;
        macro_rules! merge_f64 {
            ($field:ident) => {
                if let Some(v) = hints.get(stringify!($field)).and_then(|v| v.as_f64()) {
                    h.$field = v;
                }
            };
        }
        merge_f64!(hint_height);
        merge_f64!(hint_width_padding);
        merge_f64!(hint_font_size);
        merge_f64!(hint_font_r);
        merge_f64!(hint_font_g);
        merge_f64!(hint_font_b);
        merge_f64!(hint_font_a);
        merge_f64!(hint_first_font_r);
        merge_f64!(hint_first_font_g);
        merge_f64!(hint_first_font_b);
        merge_f64!(hint_first_font_a);
        merge_f64!(hint_first_font_size_boost);
        merge_f64!(hint_overlap_threshold);
        merge_f64!(hint_pressed_font_r);
        merge_f64!(hint_pressed_font_g);
        merge_f64!(hint_pressed_font_b);
        merge_f64!(hint_pressed_font_a);
        merge_f64!(hint_background_r);
        merge_f64!(hint_background_g);
        merge_f64!(hint_background_b);
        merge_f64!(hint_background_a);
        merge_f64!(hint_border_r);
        merge_f64!(hint_border_g);
        merge_f64!(hint_border_b);
        merge_f64!(hint_border_a);
        merge_f64!(hint_border_width);
        merge_f64!(hint_corner_radius);
        merge_f64!(hint_shadow_r);
        merge_f64!(hint_shadow_g);
        merge_f64!(hint_shadow_b);
        merge_f64!(hint_shadow_a);
        merge_f64!(hint_shadow_offset_x);
        merge_f64!(hint_shadow_offset_y);

        if let Some(face) = hints.get("hint_font_face").and_then(|v| v.as_str()) {
            h.hint_font_face = face.into();
        }
        if let Some(v) = hints.get("hint_upercase").and_then(|v| v.as_bool()) {
            h.hint_upercase = v;
        }
        if let Some(v) = hints.get("hint_shadow").and_then(|v| v.as_bool()) {
            h.hint_shadow = v;
        }
    }
}
