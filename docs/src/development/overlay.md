# src/overlay/

## src/overlay/mod.rs

### Public types

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveHook {
    Start,
    End,
}
```

```rust
#[derive(Debug, Clone)]
pub struct MouseAction {
    pub action: String,
    pub x: i32,
    pub y: i32,
    pub end_x: i32,
    pub end_y: i32,
    pub button: u32,
    pub repeat: u32,
    pub hunt_continue: bool,
    pub drag_fullscreen: bool,
}
```

### Private state

```rust
struct OverlayState {
    config: Config,
    hints: HashMap<String, usize>,
    children: Vec<Child>,
    typed: String,
    mouse_action: Rc<RefCell<Option<MouseAction>>>,
    window_size: (f64, f64),
    window_origin: (i32, i32),
    hunt: bool,
    hunt_exit_next: bool,
    text_selection_mode: bool,
    advanced_mode: bool,
    active_hook: ActiveHook,
    selection_start_child: Option<usize>,
    selection_start_offset_x: f64,
    selection_start_offset_y: f64,
    selection_end_child: Option<usize>,
    selection_end_offset_x: f64,
    selection_end_offset_y: f64,
    consumed_hints: Vec<usize>,
    double_click_mode: bool,
    drag_mode: bool,
    pulse_bright_remaining: u32,
    drag_advanced_mode: bool,
    drag_source_pos: Option<(f64, f64)>,
    drag_source_size: (f64, f64),
    drag_dest_child: Option<usize>,
    drag_source_offset_x: f64,
    drag_source_offset_y: f64,
    drag_dest_offset_x: f64,
    drag_dest_offset_y: f64,
}
```

### Public functions

```rust
pub fn show_overlay(
    config: &Config,
    hints: &HashMap<String, usize>,
    children: &[Child],
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    preset_drag_source: Option<(i32, i32)>,
) -> Option<MouseAction>
```

Creates a transparent GTK3 popup, draws hints, captures keyboard, runs `gtk::main()`.
Returns `MouseAction` on user input, `None` if dismissed.

### Private functions

```rust
fn select_position(
    child: &Child,
    start: bool,
    pad_left: f64,
    pad_right: f64,
) -> (i32, i32)
```

Computes screen coordinates for selection action:
- `start = true`: `child.left - pad_left × width`, `child.top + height / 2`
- `start = false`: `child.right + pad_right × width`, `child.top + height / 2`

### Overlay setup

1. `gtk::init()`
2. Create `gtk::Window::new(Popup)`:
   - `app_paintable = true`, `decorated = false`
   - `skip_taskbar_hint = true`, `skip_pager_hint = true`
   - `accept_focus = false`, `can_focus = false`
   - `type_hint = Notification`
   - RGBA visual for transparency
3. Add `DrawingArea` for cairo rendering
4. Initialize `OverlayState`
5. `window.realize()` then:
   - `set_override_redirect(true)`
   - `move_resize(x + overlay_x_offset, y + overlay_y_offset, width, height)`
6. Connect signals: draw, key press, button press, show, destroy

### Timers

| Timer | Duration | Trigger |
|---|---|---|
| Hunt idle | 10s inactivity | Dismiss overlay |
| Safety | 5s (extended in selection/drag) | Force `main_quit` |
| Pulse | `marker_pulse_interval_ms` (16-500ms) | `queue_draw` if markers visible |

### Key event state machine

```
1. Escape         → dismiss overlay
2. Ctrl (hunt)    → set hunt_exit_next = true
3. text_select_key (/) → toggle text selection mode
4. text_select_key again with start placed → toggle advanced mode
5. double_click_key (Alt) → toggle double-click mode
6. Shift after source placed → fullscreen drag re-scan
7. drag_key (Shift) → toggle drag mode
8. advanced_modifier / per-mode key → toggle advanced/drag_advanced
9. Tab → switch active hook (Start ↔ End)
10. Enter (drag advanced) → confirm drag, dismiss
11. Enter (advanced select) → confirm selection, dismiss
12. Arrow keys → nudge active hook position
13. Unicode char → append to typed, match against hints
    - Exact match → set end marker (selection) / fire action (normal/drag)
    - Prefix match → redraw with matching hints highlighted
    - No match → clear typed
```

## src/overlay/drawing.rs

### Public functions

```rust
pub fn draw_hints(
    cr: &Context,
    config: &Config,
    hints: &HashMap<String, usize>,
    children: &[Child],
    typed: &str,
    consumed_hints: &[usize],
    text_selection_mode: bool,
    selection_start_child: Option<usize>,
    selection_start_offset_x: f64,
    selection_start_offset_y: f64,
    selection_end_child: Option<usize>,
    selection_end_offset_x: f64,
    selection_end_offset_y: f64,
    advanced_mode: bool,
    active_hook: ActiveHook,
    double_click_mode: bool,
    drag_mode: bool,
    drag_advanced_mode: bool,
    drag_source_pos: Option<(f64, f64)>,
    drag_source_size: (f64, f64),
    drag_source_offset_x: f64,
    drag_source_offset_y: f64,
    drag_dest_child: Option<usize>,
    drag_dest_offset_x: f64,
    drag_dest_offset_y: f64,
    window_origin: (i32, i32),
    pulse_bright_remaining: u32,
    marker_bright_duration_ticks: u32,
    drag_marker_square: bool,
    drag_marker_size: f64,
    show_text_boxes: bool,
    show_bfs_boxes: bool,
    text_selection_show_boxes: bool,
    drag_show_boxes: bool,
    window_size: (f64, f64),
)
```

### Drawing order

1. Clear to transparent
2. Hint label boxes (shadow → background → border → per-character text)
3. Spotlight radial gradient holes (around matching hints)
4. Spotlight selection rectangle (between markers in advanced mode)
5. Text selection markers (start: red, end: orange)
6. Drag markers (source: red dot, dest: green dot)
7. Text selection bounding boxes (blue)
8. Drag mode bounding boxes (green)
9. Dev debug: BFS component boxes (red borders)
10. Dev debug: Text word boxes (blue borders)
11. Dev debug: Zone grid boundaries

### Private functions

```rust
fn draw_rounded_rect(
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    r: f64,
)
```

Draws a rounded rectangle path. `r` clamped to `min(w/2, h/2)`.

```rust
fn overlap_fraction(
    a: (f64, f64, f64, f64),
    b: (f64, f64, f64, f64),
) -> f64
```

Returns `intersection_area / min(area_a, area_b)`. Used for hint label overlap culling.
