pub mod drawing;

use crate::child::{Child, ChildKind};
use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveHook { Start, End }

use gdk::prelude::*;
use gtk::prelude::*;
use gtk::glib::translate::IntoGlib;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// State shared between the overlay window and its callbacks.
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
    drag_advanced_mode: bool,
    drag_source_pos: Option<(f64, f64)>,
    drag_source_size: (f64, f64),
    drag_dest_child: Option<usize>,
    drag_source_offset_x: f64,
    drag_source_offset_y: f64,
    drag_dest_offset_x: f64,
    drag_dest_offset_y: f64,
}

/// Action to perform after selecting a hint.
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

/// Display the hint overlay window and run the GTK main loop.
///
/// Returns the mouse action to perform (if any).
pub fn show_overlay(
    config: &Config,
    hints: &HashMap<String, usize>,
    children: &[Child],
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    preset_drag_source: Option<(i32, i32)>,
) -> Option<MouseAction> {
    let is_fullscreen = preset_drag_source.is_some();
    gtk::init().expect("Failed to initialize GTK");

    let window = gtk::Window::new(gtk::WindowType::Popup);
    window.set_app_paintable(true);
    window.set_decorated(false);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);
    window.set_accept_focus(false);
    window.set_can_focus(false);
    window.set_type_hint(gdk::WindowTypeHint::Notification);

    // RGBA visual for transparency
    if let Some(screen) = gtk::prelude::GtkWindowExt::screen(&window) {
        if let Some(visual) = screen.rgba_visual() {
            window.set_visual(Some(&visual));
        }
    }

    window.set_default_size(width, height);

    let drawing_area = gtk::DrawingArea::new();
    window.add(&drawing_area);

    let mouse_action: Rc<RefCell<Option<MouseAction>>> = Rc::new(RefCell::new(None));
    let dismissed: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let activity_count: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));

    let state = Rc::new(RefCell::new(OverlayState {
        config: config.clone(),
        hints: hints.clone(),
        children: children.to_vec(),
        typed: String::new(),
        mouse_action: mouse_action.clone(),
        window_size: (width as f64, height as f64),
        window_origin: (x, y),
        hunt: config.dev.hunt,
        hunt_exit_next: false,
        text_selection_mode: false,
        advanced_mode: false,
        active_hook: ActiveHook::Start,
        selection_start_child: None,
        selection_start_offset_x: 0.0,
        selection_start_offset_y: 0.0,
        selection_end_child: None,
        selection_end_offset_x: 0.0,
        selection_end_offset_y: 0.0,
        consumed_hints: Vec::new(),
        double_click_mode: false,
        drag_mode: preset_drag_source.is_some(),
        drag_advanced_mode: false,
        drag_source_pos: preset_drag_source.map(|(x, y)| (x as f64, y as f64)),
        drag_source_size: (0.0, 0.0),
        drag_dest_child: None,
        drag_source_offset_x: 0.0,
        drag_source_offset_y: 0.0,
        drag_dest_offset_x: 0.0,
        drag_dest_offset_y: 0.0,
    }));

    // Draw handler
    let state_draw = state.clone();
    drawing_area.connect_draw(move |_, cr| {
        let st = state_draw.borrow();
        drawing::draw_hints(cr, &st.config, &st.hints, &st.children, &st.typed, &st.consumed_hints,
            st.text_selection_mode, st.selection_start_child,
            st.selection_start_offset_x, st.selection_start_offset_y,
            st.selection_end_child, st.selection_end_offset_x, st.selection_end_offset_y,
            st.advanced_mode, st.active_hook, st.double_click_mode,
            st.drag_mode, st.drag_advanced_mode,
            st.drag_source_pos, st.drag_source_size, st.drag_source_offset_x, st.drag_source_offset_y,
            st.drag_dest_child, st.drag_dest_offset_x, st.drag_dest_offset_y,
            st.window_origin,
            st.window_size);
        gtk::glib::Propagation::Stop
    });

    // Mouse click → dismiss overlay (safety net when grab fails)
    let dismissed_button = dismissed.clone();
    drawing_area.connect_button_press_event(move |_, _| {
        if *dismissed_button.borrow() {
            return gtk::glib::Propagation::Stop;
        }
        *dismissed_button.borrow_mut() = true;
        gtk::main_quit();
        gtk::glib::Propagation::Stop
    });

    // Window-level mouse click → dismiss overlay (broader safety net for grab failures)
    let dismissed_win = dismissed.clone();
    window.connect_button_press_event(move |_, _| {
        if *dismissed_win.borrow() {
            return gtk::glib::Propagation::Stop;
        }
        *dismissed_win.borrow_mut() = true;
        gtk::main_quit();
        gtk::glib::Propagation::Stop
    });

    // Key press handler
    let state_key = state.clone();
    let da_clone = drawing_area.clone();
    let dismissed_key = dismissed.clone();
    let activity_key = activity_count.clone();
    window.connect_key_press_event(move |w, event| {
        *activity_key.borrow_mut() += 1;
        if *dismissed_key.borrow() {
            return gtk::glib::Propagation::Stop;
        }

        let keyval = event.keyval();
        let modifier = event.state();

        let mut st = state_key.borrow_mut();

        // Escape → exit
        if keyval.into_glib() as u32 == st.config.exit_key {
            *dismissed_key.borrow_mut() = true;
            if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
                seat.ungrab();
            }
            w.hide();
            gtk::main_quit();
            return gtk::glib::Propagation::Stop;
        }

        // Ctrl in hunt mode → next hit will be the final one
        if st.hunt && (keyval == gdk::keys::constants::Control_L || keyval == gdk::keys::constants::Control_R) {
            st.hunt_exit_next = true;
            da_clone.queue_draw();
            return gtk::glib::Propagation::Stop;
        }

        // Block mode switching when a selection/drag is already in progress
        let has_active_selection = st.selection_start_child.is_some() || st.drag_source_pos.is_some();

        // Text selection mode trigger (when typed is empty)
        if st.typed.is_empty() && !st.text_selection_mode
            && keyval.into_glib() as u32 == st.config.text_select_key
            && !has_active_selection
        {
            st.text_selection_mode = true;
            st.advanced_mode = false;
            st.selection_start_offset_x = 0.0;
            st.selection_start_offset_y = 0.0;
            st.double_click_mode = false;
            da_clone.queue_draw();
            return gtk::glib::Propagation::Stop;
        }

        // Toggle off text selection mode, or enter advanced mode if start is placed
        if st.typed.is_empty() && st.text_selection_mode
            && keyval.into_glib() as u32 == st.config.text_select_key
        {
            if st.selection_start_child.is_some() {
                // Start placed → toggle advanced mode
                st.advanced_mode = !st.advanced_mode;
                da_clone.queue_draw();
                return gtk::glib::Propagation::Stop;
            }
            // No start placed → exit text selection mode entirely
            st.text_selection_mode = false;
            st.advanced_mode = false;
            st.selection_start_child = None;
            st.selection_start_offset_x = 0.0;
            st.selection_start_offset_y = 0.0;
            st.selection_end_child = None;
            st.selection_end_offset_x = 0.0;
            st.selection_end_offset_y = 0.0;
            st.consumed_hints.clear();
            da_clone.queue_draw();
            return gtk::glib::Propagation::Stop;
        }

        // Double-click mode toggle
        if st.typed.is_empty()
            && keyval.into_glib() as u32 == st.config.double_click_key
            && !has_active_selection
        {
            st.double_click_mode = !st.double_click_mode;
            if st.double_click_mode {
                st.text_selection_mode = false;
                st.advanced_mode = false;
                st.selection_start_child = None;
                st.selection_start_offset_x = 0.0;
                st.selection_start_offset_y = 0.0;
                st.selection_end_child = None;
                st.selection_end_offset_x = 0.0;
                st.selection_end_offset_y = 0.0;
                st.consumed_hints.clear();
            }
            da_clone.queue_draw();
            return gtk::glib::Propagation::Stop;
        }

        // Shift after source → fullscreen re-scan (cross-window drag)
        if st.drag_mode && st.drag_source_pos.is_some()
            && keyval.into_glib() as u32 == st.config.drag_key
        {
            let (sx, sy) = st.drag_source_pos.unwrap();
            *dismissed_key.borrow_mut() = true;
            *st.mouse_action.borrow_mut() = Some(MouseAction {
                action: "drag".to_string(),
                x: sx as i32, y: sy as i32, end_x: 0, end_y: 0,
                button: 1, repeat: 1, hunt_continue: false,
                drag_fullscreen: true,
            });
            if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
                seat.ungrab();
            }
            w.hide();
            gtk::main_quit();
            return gtk::glib::Propagation::Stop;
        }

        // Drag mode toggle (only when source not yet placed)
        if st.typed.is_empty()
            && keyval.into_glib() as u32 == st.config.drag_key
            && !has_active_selection
        {
            log::debug!("drag toggle: drag_mode was {}, toggle now", st.drag_mode);
            st.drag_mode = !st.drag_mode;
            if st.drag_mode {
                st.text_selection_mode = false;
                st.advanced_mode = false;
                st.double_click_mode = false;
                st.selection_start_child = None;
                st.consumed_hints.clear();
            } else {
                st.drag_source_pos = None;
                st.drag_dest_child = None;
                st.consumed_hints.clear();
            }
            da_clone.queue_draw();
            return gtk::glib::Propagation::Stop;
        }

        // Advanced mode toggle — checks global modifier + per-mode keys + built-in defaults
        let k = keyval.into_glib() as u32;
        let is_ctrl = keyval == gdk::keys::constants::Control_L || keyval == gdk::keys::constants::Control_R;
        let global = st.config.advanced_modifier != 0 && k == st.config.advanced_modifier;
        let text_adv = st.config.hints.text_select_advanced_key;
        let drag_adv = st.config.hints.drag_advanced_key;

        // Text selection advanced mode
        if st.text_selection_mode && st.selection_start_child.is_some() {
            if global || (text_adv != 0 && k == text_adv) {
                st.advanced_mode = !st.advanced_mode;
                da_clone.queue_draw();
                return gtk::glib::Propagation::Stop;
            }
        }

        // Drag advanced mode
        if st.drag_mode && st.drag_source_pos.is_some() {
            if global || (drag_adv != 0 && k == drag_adv) || is_ctrl {
                st.drag_advanced_mode = !st.drag_advanced_mode;
                da_clone.queue_draw();
                return gtk::glib::Propagation::Stop;
            }
        }

        // Tab switches active hook in advanced mode (text selection or drag)
        if (st.advanced_mode || st.drag_advanced_mode) && keyval == gdk::keys::constants::Tab {
            st.active_hook = match st.active_hook {
                ActiveHook::Start => ActiveHook::End,
                ActiveHook::End => ActiveHook::Start,
            };
            da_clone.queue_draw();
            return gtk::glib::Propagation::Stop;
        }

        // Enter confirms drag in drag advanced mode
        if st.drag_advanced_mode && st.drag_source_pos.is_some() && st.drag_dest_child.is_some()
            && keyval.into_glib() as u32 == 65293
        {
            let (sx_base, sy_base) = st.drag_source_pos.unwrap();
            let dst = &st.children[st.drag_dest_child.unwrap()];
            let sx = (sx_base + st.drag_source_offset_x * st.drag_source_size.0.max(1.0)) as i32;
            let sy = (sy_base + st.drag_source_offset_y * st.drag_source_size.1.max(1.0)) as i32;
            let dx = (dst.absolute_position.0 + dst.width / 2.0 + st.drag_dest_offset_x * dst.width) as i32;
            let dy = (dst.absolute_position.1 + dst.height / 2.0 + st.drag_dest_offset_y * dst.height) as i32;

            *dismissed_key.borrow_mut() = true;
            *st.mouse_action.borrow_mut() = Some(MouseAction {
                action: "drag".to_string(),
                x: sx, y: sy, end_x: dx, end_y: dy,
                button: 1, repeat: 1, hunt_continue: false, drag_fullscreen: false,
            });
            if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
                seat.ungrab();
            }
            w.hide();
            gtk::main_quit();
            return gtk::glib::Propagation::Stop;
        }

        // Enter confirms selection in advanced mode (text selection)
        if st.advanced_mode && st.selection_start_child.is_some() && st.selection_end_child.is_some()
            && keyval.into_glib() as u32 == 65293 // GDK_KEY_Return
        {
            let start_child = &st.children[st.selection_start_child.unwrap()];
            let end_child = &st.children[st.selection_end_child.unwrap()];
            let pad_l = st.config.hints.text_select_padding_left;
            let pad_r = st.config.hints.text_select_padding_right;
            let (sx, sy) = select_position(start_child, true, pad_l, pad_r);
            let sx = sx + (st.selection_start_offset_x * start_child.width) as i32;
            let sy = sy + (st.selection_start_offset_y * start_child.height) as i32;
            let (ex, ey) = select_position(end_child, false, pad_l, pad_r);
            let ex = ex + (st.selection_end_offset_x * end_child.width) as i32;
            let ey = ey + (st.selection_end_offset_y * end_child.height) as i32;

            *dismissed_key.borrow_mut() = true;
            *st.mouse_action.borrow_mut() = Some(MouseAction {
                action: "select".to_string(),
                x: sx, y: sy, end_x: ex, end_y: ey,
                button: 1, repeat: 1, hunt_continue: false, drag_fullscreen: false,
            });
            if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
                seat.ungrab();
            }
            w.hide();
            gtk::main_quit();
            return gtk::glib::Propagation::Stop;
        }

        // Arrow keys nudge the active hook (text selection or drag mode)
        let has_any_hook = match st.active_hook {
            ActiveHook::Start => st.selection_start_child.is_some()
                || st.drag_source_pos.is_some(),
            ActiveHook::End => (st.advanced_mode && st.selection_end_child.is_some())
                || (st.drag_advanced_mode && st.drag_dest_child.is_some()),
        };
        if has_any_hook && (st.text_selection_mode || st.drag_mode) {
            let step_x = if modifier.contains(gdk::ModifierType::SHIFT_MASK) {
                st.config.hints.text_select_nudge_step_shift_x
            } else {
                st.config.hints.text_select_nudge_step_x
            };
            let step_y = if modifier.contains(gdk::ModifierType::SHIFT_MASK) {
                st.config.hints.text_select_nudge_step_shift_y
            } else {
                st.config.hints.text_select_nudge_step_y
            };
            let moved = match (st.active_hook, keyval) {
                (ActiveHook::Start, k) if k == gdk::keys::constants::Left && st.text_selection_mode => { st.selection_start_offset_x -= step_x; true }
                (ActiveHook::Start, k) if k == gdk::keys::constants::Right && st.text_selection_mode => { st.selection_start_offset_x += step_x; true }
                (ActiveHook::Start, k) if k == gdk::keys::constants::Up && st.text_selection_mode => { st.selection_start_offset_y -= step_y; true }
                (ActiveHook::Start, k) if k == gdk::keys::constants::Down && st.text_selection_mode => { st.selection_start_offset_y += step_y; true }
                (ActiveHook::End, k) if k == gdk::keys::constants::Left && st.text_selection_mode => { st.selection_end_offset_x -= step_x; true }
                (ActiveHook::End, k) if k == gdk::keys::constants::Right && st.text_selection_mode => { st.selection_end_offset_x += step_x; true }
                (ActiveHook::End, k) if k == gdk::keys::constants::Up && st.text_selection_mode => { st.selection_end_offset_y -= step_y; true }
                (ActiveHook::End, k) if k == gdk::keys::constants::Down && st.text_selection_mode => { st.selection_end_offset_y += step_y; true }
                (ActiveHook::Start, k) if k == gdk::keys::constants::Left && st.drag_mode => { st.drag_source_offset_x -= step_x; true }
                (ActiveHook::Start, k) if k == gdk::keys::constants::Right && st.drag_mode => { st.drag_source_offset_x += step_x; true }
                (ActiveHook::Start, k) if k == gdk::keys::constants::Up && st.drag_mode => { st.drag_source_offset_y -= step_y; true }
                (ActiveHook::Start, k) if k == gdk::keys::constants::Down && st.drag_mode => { st.drag_source_offset_y += step_y; true }
                (ActiveHook::End, k) if k == gdk::keys::constants::Left && st.drag_mode => { st.drag_dest_offset_x -= step_x; true }
                (ActiveHook::End, k) if k == gdk::keys::constants::Right && st.drag_mode => { st.drag_dest_offset_x += step_x; true }
                (ActiveHook::End, k) if k == gdk::keys::constants::Up && st.drag_mode => { st.drag_dest_offset_y -= step_y; true }
                (ActiveHook::End, k) if k == gdk::keys::constants::Down && st.drag_mode => { st.drag_dest_offset_y += step_y; true }
                _ => false,
            };
            if moved {
                da_clone.queue_draw();
                return gtk::glib::Propagation::Stop;
            }
        }

        // Get the character pressed
        if let Some(ch) = gdk::keys::Key::from(keyval).to_unicode() {
            let ch_lower = ch.to_lowercase().next().unwrap_or(ch);
            st.typed.push(ch_lower);

            // With all 2-char hints, check for exact match
            if let Some(&child_idx) = st.hints.get(&st.typed) {
                if st.text_selection_mode {
                    if let Some(start_idx) = st.selection_start_child {
                        if st.advanced_mode {
                            // ── Advanced mode: second hint places end marker ──
                            if st.selection_end_child.is_some() {
                                // Replace end marker
                                let old = st.selection_end_child.take();
                                if let Some(old_idx) = old {
                                    st.consumed_hints.retain(|&x| x != old_idx);
                                }
                            }
                            st.selection_end_child = Some(child_idx);
                            st.selection_end_offset_x = 0.0;
                            st.selection_end_offset_y = 0.0;
                            st.active_hook = ActiveHook::End;
                            st.consumed_hints.push(child_idx);
                            st.typed.clear();
                            da_clone.queue_draw();
                            return gtk::glib::Propagation::Stop;
                        } else {
                            // ── Normal text selection: second hint fires action
                            let start_child = &st.children[start_idx];
                            let end_child = &st.children[child_idx];

                            let pad_l = st.config.hints.text_select_padding_left;
                            let pad_r = st.config.hints.text_select_padding_right;
                            let (sx, sy) = select_position(start_child, true, pad_l, pad_r);
                            let sx = sx + (st.selection_start_offset_x * start_child.width) as i32;
                            let sy = sy + (st.selection_start_offset_y * start_child.height) as i32;
                            let (ex, ey) = select_position(end_child, false, pad_l, pad_r);

                            *dismissed_key.borrow_mut() = true;
                            *st.mouse_action.borrow_mut() = Some(MouseAction {
                                action: "select".to_string(),
                                x: sx, y: sy, end_x: ex, end_y: ey,
                    button: 1, repeat: 1, hunt_continue: false, drag_fullscreen: false,
                });

                        if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
                                seat.ungrab();
                            }
                            w.hide();
                            gtk::main_quit();
                            return gtk::glib::Propagation::Stop;
                        }
                    } else {
                        // First hint → store as selection start, keep overlay
                        st.selection_start_child = Some(child_idx);
                        st.selection_start_offset_x = 0.0;
                        st.selection_start_offset_y = 0.0;
                        st.active_hook = ActiveHook::Start;
                        st.consumed_hints.push(child_idx);
                        st.typed.clear();
                        da_clone.queue_draw();
                        return gtk::glib::Propagation::Stop;
                    }
                }

                if st.drag_mode {
                    let adv = st.drag_advanced_mode;
                    let fscreen_def = st.config.hints.drag_fullscreen_default;
                    let is_fs = is_fullscreen;
                    let pos = st.drag_source_pos.is_some();
                    log::debug!("DRAG HIT: pos={} adv={} fscreen_def={} is_fs={} text_sel={}",
                        pos, adv, fscreen_def, is_fs, st.text_selection_mode);
                    log::debug!("DRAG HIT: will_fullscreen={}, will_marker={}, will_exec={}",
                        pos && !adv && fscreen_def && !is_fs,
                        pos && adv,
                        pos && !adv && !(fscreen_def && !is_fs));
                    if st.drag_source_pos.is_some() {
                        // Always place dest marker (basic or advanced mode)
                        if let Some(old) = st.drag_dest_child.take() {
                            st.consumed_hints.retain(|&x| x != old);
                        }
                        st.drag_dest_child = Some(child_idx);
                        st.drag_dest_offset_x = 0.0;
                        st.drag_dest_offset_y = 0.0;
                        st.active_hook = ActiveHook::End;
                        st.consumed_hints.push(child_idx);
                        st.typed.clear();
                        if st.drag_advanced_mode {
                            da_clone.queue_draw();
                            return gtk::glib::Propagation::Stop;
                        } else {
                            // Basic mode: execute drag immediately
                            let (sx_base, sy_base) = st.drag_source_pos.unwrap();
                            let dst = &st.children[child_idx];
                            let sx = (sx_base + st.drag_source_offset_x * st.drag_source_size.0.max(1.0)) as i32;
                            let sy = (sy_base + st.drag_source_offset_y * st.drag_source_size.1.max(1.0)) as i32;
                            let dx = (dst.absolute_position.0 + dst.width / 2.0) as i32;
                            let dy = (dst.absolute_position.1 + dst.height / 2.0) as i32;

                            *dismissed_key.borrow_mut() = true;
                            *st.mouse_action.borrow_mut() = Some(MouseAction {
                                action: "drag".to_string(),
                                x: sx, y: sy, end_x: dx, end_y: dy,
                                button: 1, repeat: 1, hunt_continue: false,
                                drag_fullscreen: false,
                            });
                            if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
                                seat.ungrab();
                            }
                            w.hide();
                            gtk::main_quit();
                            return gtk::glib::Propagation::Stop;
                        }
                    } else {
                        // First hint → store as drag source (always stay)
                        log::debug!("drag first hint: child_idx={}, children.len={}", child_idx, st.children.len());
                        let src = st.children[child_idx].clone();
                        st.drag_source_pos = Some((src.absolute_position.0 + src.width / 2.0, src.absolute_position.1 + src.height / 2.0));
                        st.drag_source_size = (src.width, src.height);
                        st.drag_source_offset_x = 0.0;
                        st.drag_source_offset_y = 0.0;
                        st.active_hook = ActiveHook::Start;
                        st.consumed_hints.push(child_idx);
                        st.typed.clear();
                        da_clone.queue_draw();
                        return gtk::glib::Propagation::Stop;
                    }
                }

                // ── Normal mode: click / hover / grab ────────────────────
                let child = &st.children[child_idx];
                let click_x = child.absolute_position.0 as i32 + (child.width as i32 / 2);
                let click_y = child.absolute_position.1 as i32 + (child.height as i32 / 2);

                let double = st.double_click_mode;
                if double {
                    st.double_click_mode = false;
                }
                let (action, button, repeat) = if modifier.contains(gdk::ModifierType::CONTROL_MASK) {
                    ("hover".to_string(), 1u32, 1u32)
                } else if double {
                    ("click".to_string(), 1u32, 2u32)
                } else {
                    ("click".to_string(), 1u32, 1u32)
                };

                *dismissed_key.borrow_mut() = true;
                *st.mouse_action.borrow_mut() = Some(MouseAction {
                    action, x: click_x, y: click_y,
                    end_x: 0, end_y: 0, button, repeat,
                    hunt_continue: st.hunt && !st.hunt_exit_next,
                    drag_fullscreen: false,
                });

                if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
                    seat.ungrab();
                }
                w.hide();
                gtk::main_quit();
                return gtk::glib::Propagation::Stop;
            }

            // Check if any hints still match the prefix
            let prefix = &st.typed;
            let any_match = st.hints.keys().any(|k| k.starts_with(prefix.as_str()));

            if !any_match {
                st.typed.clear();
            }

            da_clone.queue_draw();
        }

        gtk::glib::Propagation::Stop
    });

    // Realize early to set position, size, and override_redirect before mapping
    window.realize();
    if let Some(gdk_win) = window.window() {
        gdk_win.set_override_redirect(true);
        gdk_win.move_resize(x + config.overlay_x_offset, y + config.overlay_y_offset, width, height);
    }

    // Grab keyboard on show
    window.connect_show(move |w| {
        let gdk_win = match w.window() {
            Some(win) => win,
            None => {
                log::error!("No GdkWindow available for grab");
                return;
            }
        };
        // Make overlay mouse‑transparent so underlying app keeps hover state
        let region = gdk::cairo::Region::create();
        gdk_win.input_shape_combine_region(&region, 0, 0);
        if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
            let status = seat.grab(
                &gdk_win,
                gdk::SeatCapabilities::KEYBOARD,
                true,
                None,
                None,
                None,
            );
            match status {
                gdk::GrabStatus::Success => {
                    log::debug!("Keyboard grab succeeded");
                }
                other => {
                    log::error!("Keyboard grab returned {:?} — overlay may not receive keyboard input; use mouse click to dismiss or wait for 5s timeout", other);
                }
            }
        } else {
            log::error!("No default seat available for grab");
        }
    });

    // Ensure main loop exits if window is destroyed externally
    window.connect_destroy(|w| {
        if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
            seat.ungrab();
        }
        gtk::main_quit();
    });

    // Safety net / idle timeout
    if config.dev.hunt {
        let act_idle = activity_count.clone();
        let dismissed_idle = dismissed.clone();
        let mut last_act = 0u64;
        let mut idle_secs = 0u64;
        gtk::glib::timeout_add_seconds_local(1, move || {
            if *dismissed_idle.borrow() {
                return gtk::glib::ControlFlow::Break;
            }
            let current = *act_idle.borrow();
            if current != last_act {
                last_act = current;
                idle_secs = 0;
            } else {
                idle_secs += 1;
            }
            if idle_secs >= 10 {
                log::warn!("Hunt idle 10s — dismissing");
                gtk::main_quit();
                return gtk::glib::ControlFlow::Break;
            }
            gtk::glib::ControlFlow::Continue
        });
    } else {
        let state_timeout = state.clone();
        let dismissed_timeout = dismissed.clone();
        gtk::glib::timeout_add_seconds_local(5, move || {
            if *dismissed_timeout.borrow() {
                return gtk::glib::ControlFlow::Break;
            }
            let st = state_timeout.borrow();
            if st.text_selection_mode || st.advanced_mode || st.selection_start_child.is_some()
                || st.drag_mode || st.drag_source_pos.is_some() {
                return gtk::glib::ControlFlow::Continue;
            }
            log::warn!("Overlay main loop did not exit within 5s — forcing quit");
            gtk::main_quit();
            gtk::glib::ControlFlow::Break
        });
    }

    // Pulse animation for active hook in advanced mode
    let state_pulse = state.clone();
    let da_pulse = drawing_area.clone();
    let dismissed_pulse = dismissed.clone();
    let pulse_interval = config.hints.marker_pulse_interval_ms.max(16).min(500);
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(pulse_interval), move || {
        if *dismissed_pulse.borrow() {
            return gtk::glib::ControlFlow::Break;
        }
        let pst = state_pulse.borrow();
        if pst.advanced_mode || pst.drag_advanced_mode
            || (pst.text_selection_mode && pst.selection_start_child.is_some())
            || (pst.drag_mode && pst.drag_source_pos.is_some()) {
            da_pulse.queue_draw();
        }
        gtk::glib::ControlFlow::Continue
    });

    window.show_all();
    gtk::main();

    let result = mouse_action.borrow().clone();
    result
}

/// Compute the selection position for a child element.
///
/// For `Text` children the position snaps to the left (`start = true`) or
/// right (`start = false`) edge so that entire words are selected.
/// For `Element` children the center of the element is used.
/// `pad_left` and `pad_right` are fractions of the element's width.
fn select_position(child: &Child, start: bool, pad_left: f64, pad_right: f64) -> (i32, i32) {
    let w_off = |ratio: f64| (child.width * ratio) as i32;
    match child.kind {
        ChildKind::Text => {
            if start {
                let x = (child.absolute_position.0 as i32) - w_off(pad_left);
                let y = child.absolute_position.1 as i32 + (child.height as i32 / 2);
                (x, y)
            } else {
                let x = (child.absolute_position.0 + child.width) as i32 + w_off(pad_right);
                let y = child.absolute_position.1 as i32 + (child.height as i32 / 2);
                (x, y)
            }
        }
        ChildKind::Element => {
            let cx = child.absolute_position.0 as i32 + (child.width as i32 / 2);
            if start {
                let x = cx - w_off(pad_left);
                let y = child.absolute_position.1 as i32 + (child.height as i32 / 2);
                (x, y)
            } else {
                let x = cx + w_off(pad_right);
                let y = child.absolute_position.1 as i32 + (child.height as i32 / 2);
                (x, y)
            }
        }
    }
}