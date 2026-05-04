pub mod drawing;

use crate::child::Child;
use crate::config::Config;
use gtk::cairo::Context;
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
}

/// Action to perform after selecting a hint.
#[derive(Debug, Clone)]
pub struct MouseAction {
    pub action: String,
    pub x: i32,
    pub y: i32,
    pub button: u32,
    pub repeat: u32,
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
) -> Option<MouseAction> {
    gtk::init().expect("Failed to initialize GTK");

    let window = gtk::Window::new(gtk::WindowType::Popup);
    window.set_app_paintable(true);
    window.set_decorated(false);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);
    window.set_accept_focus(true);
    window.set_can_focus(true);

    // RGBA visual for transparency
    if let Some(screen) = gtk::prelude::GtkWindowExt::screen(&window) {
        if let Some(visual) = screen.rgba_visual() {
            window.set_visual(Some(&visual));
        }
    }

    window.move_(x + config.overlay_x_offset, y + config.overlay_y_offset);
    window.set_default_size(width, height);

    let drawing_area = gtk::DrawingArea::new();
    window.add(&drawing_area);

    let mouse_action: Rc<RefCell<Option<MouseAction>>> = Rc::new(RefCell::new(None));

    let state = Rc::new(RefCell::new(OverlayState {
        config: config.clone(),
        hints: hints.clone(),
        children: children.to_vec(),
        typed: String::new(),
        mouse_action: mouse_action.clone(),
    }));

    // Draw handler
    let state_draw = state.clone();
    drawing_area.connect_draw(move |_, cr| {
        let st = state_draw.borrow();
        drawing::draw_hints(cr, &st.config, &st.hints, &st.children, &st.typed);
        gtk::glib::Propagation::Stop
    });

    // Mouse click → dismiss overlay (safety net when grab fails)
    drawing_area.connect_button_press_event(|_, _| {
        gtk::main_quit();
        gtk::glib::Propagation::Stop
    });

    // Key press handler
    let state_key = state.clone();
    let da_clone = drawing_area.clone();
    window.connect_key_press_event(move |w, event| {
        let keyval = event.keyval();
        let modifier = event.state();

        let mut st = state_key.borrow_mut();

        // Escape → exit
        if keyval.into_glib() as u32 == st.config.exit_key {
            if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
                seat.ungrab();
            }
            gtk::main_quit();
            return gtk::glib::Propagation::Stop;
        }

        // Get the character pressed
        if let Some(ch) = gdk::keys::Key::from(keyval).to_unicode() {
            let ch_lower = ch.to_lowercase().next().unwrap_or(ch);
            st.typed.push(ch_lower);

            // With all 2-char hints, check for exact match
            if let Some(&child_idx) = st.hints.get(&st.typed) {
                let child = &st.children[child_idx];
                let click_x = child.absolute_position.0 as i32 + (child.width as i32 / 2);
                let click_y = child.absolute_position.1 as i32 + (child.height as i32 / 2);

                let (action, button, repeat) = if modifier.contains(gdk::ModifierType::CONTROL_MASK) {
                    ("hover".to_string(), 1u32, 1u32)
                } else if modifier.contains(gdk::ModifierType::MOD1_MASK) {
                    ("grab".to_string(), 1, 1)
                } else {
                    ("click".to_string(), 1, 1)
                };

                *st.mouse_action.borrow_mut() = Some(MouseAction {
                    action,
                    x: click_x,
                    y: click_y,
                    button,
                    repeat,
                });

                // Release keyboard grab
                if let Some(seat) = gtk::prelude::WidgetExt::display(w).default_seat() {
                    seat.ungrab();
                }
                
                // Hide window immediately
                w.hide();

                gtk::main_quit();
                return gtk::glib::Propagation::Stop;
            }

            // Check if any hints still match the prefix
            let prefix = &st.typed;
            let any_match = st.hints.keys().any(|k| k.starts_with(prefix.as_str()));

            if !any_match {
                // No match — reset
                st.typed.clear();
            }

            da_clone.queue_draw();
        }

        gtk::glib::Propagation::Stop
    });

    // Grab keyboard on show
    window.connect_show(move |w| {
        let gdk_win = match w.window() {
            Some(win) => win,
            None => {
                log::error!("No GdkWindow available for grab");
                return;
            }
        };
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
                    log::warn!("Keyboard grab returned {:?}", other);
                }
            }
        } else {
            log::error!("No default seat available for grab");
        }
    });

    // Ensure main loop exits if window is destroyed externally
    window.connect_destroy(|_| {
        gtk::main_quit();
    });

    window.show_all();
    gtk::main();

    let result = mouse_action.borrow().clone();
    result
}