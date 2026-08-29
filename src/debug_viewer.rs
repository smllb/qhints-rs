//! Live pipeline debug viewer.
//!
//! A GTK app (think "GIF-maker" UI) for tuning the imageproc detection
//! pipeline: capture the focused window (or the full screen), then adjust
//! the detection knobs and watch the pipeline stages update live.
//!
//! Layer toggles:
//! - luma / edges: base image rendering stages
//! - words: detected text-word boxes (blue)
//! - all_bfs: raw BFS components before classification (orange)
//! - final: the actual children a user would get hints for (blue = Text,
//!   green = Element, red = culled by overlap)

use crate::backend::imageproc;
use crate::child::ChildKind;
use crate::config::ApplicationRule;
use crate::filter;
use crate::window_system::{WindowInfo, WindowSystem};

use gdk::prelude::*;
use gtk::cairo;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// A colored rectangle to draw over the preview.
struct Box2D {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: (f64, f64, f64, f64),
    thick: f64,
}

/// One rendered preview frame.
struct RenderData {
    surface: cairo::ImageSurface,
    img_w: f64,
    img_h: f64,
    boxes: Vec<Box2D>,
}

struct ViewerState {
    info: Option<WindowInfo>,
    image: Option<image::RgbaImage>,
    rule: ApplicationRule,
    hint_overlap_threshold: f64,
    render: Option<RenderData>,
    zoom: f64, // 1.0 = fit; wheel multiplies
    show_luma: bool,
    show_edges: bool,
    show_words: bool,
    show_all_bfs: bool,
    show_final: bool,
    da: gtk::glib::WeakRef<gtk::DrawingArea>,
    stats_label: gtk::glib::WeakRef<gtk::Label>,
}

impl ViewerState {
    fn new() -> Self {
        Self {
            info: None,
            image: None,
            rule: ApplicationRule::default(),
            hint_overlap_threshold: 60.0,
            render: None,
            zoom: 1.0,
            show_luma: false,
            show_edges: false,
            show_words: false,
            show_all_bfs: false,
            show_final: true,
            da: gtk::glib::WeakRef::default(),
            stats_label: gtk::glib::WeakRef::default(),
        }
    }

    fn set_stats(&mut self, s: String) {
        if let Some(l) = self.stats_label.upgrade() {
            l.set_text(&s);
        }
    }
}

/// Run the detection pipeline on the stored image and rebuild the preview.
fn rerun(state: &mut ViewerState) {
    let da = match state.da.upgrade() {
        Some(d) => d,
        None => return,
    };

    state.render = None;
    let Some(img) = state.image.clone() else {
        return;
    };
    let (w, h) = img.dimensions();

    let t0 = std::time::Instant::now();
    let dyn_img = image::DynamicImage::ImageRgba8(img.clone());
    match imageproc::detect_children_debug(&dyn_img, &state.rule, 0.0, 0.0) {
        Ok(debug) => {
            let detect_ms = t0.elapsed().as_secs_f64() * 1000.0;

            // ── Composite the base image ─────────────────────────────────
            let mut base: image::RgbaImage = if state.show_luma {
                let luma = &debug.luma;
                image::RgbaImage::from_fn(w, h, |x, y| {
                    let l = luma.get_pixel(x, y)[0];
                    image::Rgba([l, l, l, 255])
                })
            } else {
                img.clone()
            };

            if state.show_edges {
                let edges = &debug.edges;
                let (ew, eh) = edges.dimensions();
                let er: image::GrayImage = if ew == w && eh == h {
                    edges.clone()
                } else {
                    image::imageops::resize(edges, w, h, image::imageops::FilterType::Nearest)
                };
                for (x, y, p) in er.enumerate_pixels() {
                    if p[0] > 0 {
                        let src = base.get_pixel(x, y);
                        let r = src.0[0] as f32 * 0.35 + 255.0 * 0.65;
                        let g = src.0[1] as f32 * 0.25;
                        let b = src.0[2] as f32 * 0.25;
                        base.put_pixel(x, y, image::Rgba([r as u8, g as u8, b as u8, 255]));
                    }
                }
            }

            let surface = surface_from_rgba(&base);

            // ── Overlay boxes ────────────────────────────────────────────
            let mut boxes: Vec<Box2D> = Vec::new();

            if state.show_words {
                for wd in &debug.words {
                    boxes.push(Box2D {
                        x: wd.relative_position.0,
                        y: wd.relative_position.1,
                        w: wd.width,
                        h: wd.height,
                        color: (0.0, 0.6, 1.0, 0.9),
                        thick: 1.2,
                    });
                }
            }

            if state.show_all_bfs {
                for c in &debug.all_bfs {
                    boxes.push(Box2D {
                        x: c.relative_position.0,
                        y: c.relative_position.1,
                        w: c.width,
                        h: c.height,
                        color: (1.0, 0.55, 0.0, 0.9),
                        thick: 1.2,
                    });
                }
            }

            let mut n_text = 0usize;
            let mut n_elem = 0usize;
            let mut n_kept = 0usize;
            if state.show_final {
                let kids = debug.children.clone();
                let before_tiny = kids.len();
                let kids = filter::filter_tiny(kids, w as f64, h as f64);
                let kept =
                    filter::cull_overlaps(&kids, filter::overlap_limit(state.hint_overlap_threshold));
                for (i, c) in kids.iter().enumerate() {
                    let (color, thick) = if kept[i] {
                        n_kept += 1;
                        match c.kind {
                            ChildKind::Text => {
                                n_text += 1;
                                ((0.0, 0.5, 1.0, 1.0), 2.0)
                            }
                            ChildKind::Element => {
                                n_elem += 1;
                                ((0.0, 0.85, 0.3, 1.0), 2.0)
                            }
                        }
                    } else {
                        ((1.0, 0.0, 0.0, 0.8), 1.0)
                    };
                    boxes.push(Box2D {
                        x: c.relative_position.0,
                        y: c.relative_position.1,
                        w: c.width,
                        h: c.height,
                        color,
                        thick,
                    });
                }

                let r = &state.rule;
                let info = state
                    .info
                    .as_ref()
                    .map(|i| i.app_name.clone())
                    .unwrap_or_default();
                state.set_stats(format!(
                    "window: {}  {w}x{h}\n\
                     detect {detect_ms:.0}ms | words {} | bfs {}\n\
                     final {} → {} | kept {} (text {n_text}, elem {n_elem}) | culled {}\n\
                     knobs: scale={:.2} canny=[{},{}] kernel={} overlap={:.0} | zoom {:.0}%",
                    info,
                    debug.words.len(),
                    debug.all_bfs.len(),
                    before_tiny,
                    kids.len(),
                    n_kept,
                    kids.len() - n_kept,
                    r.detection_scale,
                    r.canny_min_val,
                    r.canny_max_val,
                    r.kernel_size,
                    state.hint_overlap_threshold,
                    state.zoom * 100.0,
                ));
            } else {
                state.set_stats(format!(
                    "window: {}  {w}x{h}\n\
                     detect {detect_ms:.0}ms | words {} | bfs {}\n\
                     (final layer hidden)",
                    state
                        .info
                        .as_ref()
                        .map(|i| i.app_name.clone())
                        .unwrap_or_default(),
                    debug.words.len(),
                    debug.all_bfs.len(),
                ));
            }

            state.render = Some(RenderData {
                surface,
                img_w: w as f64,
                img_h: h as f64,
                boxes,
            });
        }
        Err(e) => {
            state.set_stats(format!("detection error: {}", e));
        }
    }
    da.queue_draw();
}

/// Convert an RGBA image to a cairo ARGB32 surface (premultiplied, A=255).
/// `create_for_data` boxes the buffer and ties its lifetime to the surface.
fn surface_from_rgba(img: &image::RgbaImage) -> cairo::ImageSurface {
    let (w, h) = img.dimensions();
    let mut buf = vec![0u8; w as usize * h as usize * 4];
    for (i, px) in img.pixels().enumerate() {
        let b = i * 4;
        buf[b] = px.0[2]; // B
        buf[b + 1] = px.0[1]; // G
        buf[b + 2] = px.0[0]; // R
        buf[b + 3] = 255; // A
    }
    cairo::ImageSurface::create_for_data(
        buf,
        cairo::Format::ARgb32,
        w as i32,
        h as i32,
        (w as usize * 4) as i32,
    )
    .expect("failed to create cairo surface")
}

fn capture_focused(state: &Rc<RefCell<ViewerState>>) {
    match crate::window_system::x11::X11::new() {
        Ok(ws) => {
            let info = ws.focused_window().clone();
            match imageproc::capture_window_image(&info) {
                Ok(img) => {
                    let mut st = state.borrow_mut();
                    st.info = Some(info);
                    st.image = Some(img.to_rgba8());
                    rerun(&mut st);
                }
                Err(e) => {
                    state.borrow_mut().set_stats(format!("capture error: {}", e));
                }
            }
        }
        Err(e) => {
            state
                .borrow_mut()
                .set_stats(format!("window lookup error: {}", e));
        }
    }
}

fn capture_fullscreen(state: &Rc<RefCell<ViewerState>>) {
    match crate::window_system::x11::screen_size() {
        Ok((sw, sh)) => {
            let info = WindowInfo {
                extents: (0, 0, sw, sh),
                pid: 0,
                app_name: "__screen__".into(),
            };
            match imageproc::capture_window_image(&info) {
                Ok(img) => {
                    let mut st = state.borrow_mut();
                    st.info = Some(info);
                    st.image = Some(img.to_rgba8());
                    rerun(&mut st);
                }
                Err(e) => {
                    state.borrow_mut().set_stats(format!("capture error: {}", e));
                }
            }
        }
        Err(e) => {
            state.borrow_mut().set_stats(format!("screen error: {}", e));
        }
    }
}

/// Build a compact labeled slider column and return (container, scale).
fn make_slider(
    label: &str,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
) -> (gtk::Box, gtk::Scale) {
    let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let l = gtk::Label::new(Some(label));
    l.set_halign(gtk::Align::Start);
    let sc = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, step);
    sc.set_value(value);
    sc.set_draw_value(false);
    sc.set_size_request(120, -1);
    col.pack_start(&l, false, false, 0);
    col.pack_start(&sc, false, false, 0);
    (col, sc)
}

/// Entry point — call after `gtk::init()`.
pub fn run() {
    // Readable text regardless of theme defaults.
    if let Some(screen) = gdk::Screen::default() {
        let provider = gtk::CssProvider::new();
        let _ = provider.load_from_data(
            b"window, label, button, checkbutton, scale { font-size: 14px; }",
        );
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let state = Rc::new(RefCell::new(ViewerState::new()));

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("qhints pipeline debugger");
    window.set_default_size(1500, 950);
    // Keep the target window focused so "capture focused window" works.
    window.set_accept_focus(false);
    window.set_skip_taskbar_hint(true);
    window.set_type_hint(gdk::WindowTypeHint::Utility);

    // ── Layout: horizontal control bar on top, preview fills the rest ──
    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    window.add(&root);

    let topbar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    topbar.set_margin_top(6);
    topbar.set_margin_bottom(6);
    topbar.set_margin_start(6);
    topbar.set_margin_end(6);
    root.pack_start(&topbar, false, false, 0);

    // buttons
    let buttons_col = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let b_focus = gtk::Button::with_label("Capture window");
    let b_full = gtk::Button::with_label("Capture screen");
    let b_fit = gtk::Button::with_label("Fit zoom");
    buttons_col.pack_start(&b_focus, false, false, 0);
    buttons_col.pack_start(&b_full, false, false, 0);
    buttons_col.pack_start(&b_fit, false, false, 0);
    topbar.pack_start(&buttons_col, false, false, 0);

    // stats
    let stats_col = gtk::Box::new(gtk::Orientation::Vertical, 0);
    stats_col.set_size_request(300, -1);
    let stats_label = gtk::Label::new(None);
    stats_label.set_xalign(0.0);
    stats_label.set_selectable(true);
    stats_label.set_line_wrap(true);
    stats_col.pack_start(&stats_label, false, false, 0);
    topbar.pack_start(&stats_col, false, false, 0);

    // sliders
    let sliders_h = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let (s_scale_col, s_scale) = make_slider("detection_scale", 0.5, 2.0, 0.05, 1.0);
    let (s_min_col, s_min) = make_slider("canny_min", 1.0, 80.0, 1.0, 15.0);
    let (s_max_col, s_max) = make_slider("canny_max", 10.0, 160.0, 1.0, 40.0);
    let (s_kernel_col, s_kernel) = make_slider("kernel", 1.0, 15.0, 2.0, 3.0);
    let (s_overlap_col, s_overlap) = make_slider("hint_overlap", 0.0, 100.0, 5.0, 60.0);
    for c in [&s_scale_col, &s_min_col, &s_max_col, &s_kernel_col, &s_overlap_col] {
        sliders_h.pack_start(c, false, false, 0);
    }
    topbar.pack_start(&sliders_h, false, false, 0);

    // layer toggles
    let checks_col = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let mk_check = |label: &str, active: bool| -> gtk::CheckButton {
        let cb = gtk::CheckButton::with_label(label);
        cb.set_active(active);
        cb
    };
    let cb_luma = mk_check("luma", false);
    let cb_edges = mk_check("edges", false);
    let cb_words = mk_check("words", false);
    let cb_bfs = mk_check("BFS", false);
    let cb_final = mk_check("final (T/E/culled)", true);
    for cb in [&cb_luma, &cb_edges, &cb_words, &cb_bfs, &cb_final] {
        cb.set_halign(gtk::Align::Start);
        checks_col.pack_start(cb, false, false, 0);
    }
    topbar.pack_start(&checks_col, false, false, 0);

    // ── Preview fills remaining space ────────────────────────────────────
    let da = gtk::DrawingArea::new();
    root.pack_start(&da, true, true, 0);

    {
        let mut st = state.borrow_mut();
        st.da = da.downgrade();
        st.stats_label = stats_label.downgrade();
    }

    // ── Signals ─────────────────────────────────────────────────────────
    let st = state.clone();
    b_focus.connect_clicked(move |_| capture_focused(&st));
    let st = state.clone();
    b_full.connect_clicked(move |_| capture_fullscreen(&st));
    let st = state.clone();
    b_fit.connect_clicked(move |_| {
        {
            let mut st = st.borrow_mut();
            st.zoom = 1.0;
        }
        if let Some(da) = st.borrow().da.upgrade() {
            da.queue_draw();
        }
    });

    macro_rules! connect_rule {
        ($slider:expr, $($set:tt)+) => {{
            let st = state.clone();
            $slider.connect_value_changed(move |s| {
                let mut st = st.borrow_mut();
                st.rule.$($set)+ = s.value();
                rerun(&mut st);
            });
        }};
    }
    connect_rule!(s_scale, detection_scale);
    let st = state.clone();
    s_min.connect_value_changed(move |s| {
        let mut st = st.borrow_mut();
        st.rule.canny_min_val = s.value() as i32;
        rerun(&mut st);
    });
    let st = state.clone();
    s_max.connect_value_changed(move |s| {
        let mut st = st.borrow_mut();
        st.rule.canny_max_val = s.value() as i32;
        rerun(&mut st);
    });
    let st = state.clone();
    s_kernel.connect_value_changed(move |s| {
        let mut st = st.borrow_mut();
        st.rule.kernel_size = s.value() as i32;
        rerun(&mut st);
    });

    let st = state.clone();
    s_overlap.connect_value_changed(move |s| {
        let mut st = st.borrow_mut();
        st.hint_overlap_threshold = s.value();
        rerun(&mut st);
    });

    let toggle = |cb: &gtk::CheckButton, field: &'static str| {
        let st = state.clone();
        cb.connect_toggled(move |c| {
            let mut st = st.borrow_mut();
            match field {
                "luma" => st.show_luma = c.is_active(),
                "edges" => st.show_edges = c.is_active(),
                "words" => st.show_words = c.is_active(),
                "bfs" => st.show_all_bfs = c.is_active(),
                "final" => st.show_final = c.is_active(),
                _ => {}
            }
            rerun(&mut st);
        });
    };
    toggle(&cb_luma, "luma");
    toggle(&cb_edges, "edges");
    toggle(&cb_words, "words");
    toggle(&cb_bfs, "bfs");
    toggle(&cb_final, "final");

    let state_draw = state.clone();
    da.connect_draw(move |_, cr| {
        let st = state_draw.borrow();
        let (_, _, aw, ah) = cr.clip_extents().ok().unwrap_or((0.0, 0.0, 0.0, 0.0));

        cr.set_operator(cairo::Operator::Source);
        cr.set_source_rgb(0.12, 0.12, 0.12);
        let _ = cr.paint();
        cr.set_operator(cairo::Operator::Over);

if let Some(rd) = &st.render {
            if aw > 1.0 && ah > 1.0 && rd.img_w > 0.0 && rd.img_h > 0.0 {
                let fit = (aw / rd.img_w).min(ah / rd.img_h);
                let zoom = st.zoom.clamp(0.2, 8.0);
                let scale = fit * zoom;
                let ox = (aw - rd.img_w * scale) / 2.0;
                let oy = (ah - rd.img_h * scale) / 2.0;
                cr.save().ok();
                cr.translate(ox, oy);
                cr.scale(scale, scale);
                cr.set_source_surface(&rd.surface, 0.0, 0.0).ok();
                let _ = cr.paint();
                for b in &rd.boxes {
                    cr.rectangle(b.x, b.y, b.w, b.h);
                    cr.set_source_rgba(b.color.0, b.color.1, b.color.2, b.color.3);
                    cr.set_line_width((b.thick / scale).max(0.5));
                    let _ = cr.stroke();
                }
                cr.restore().ok();
            }
        }
        gtk::glib::Propagation::Stop
    });

    // Mouse wheel zooms the preview (around center); only redraws, no re-detect.
    let state_zoom = state.clone();
    da.connect_scroll_event(move |_, ev| {
        let (_, dy) = ev.delta();
        if dy != 0.0 {
            let factor = if dy < 0.0 { 1.2 } else { 1.0 / 1.2 };
            let mut st = state_zoom.borrow_mut();
            st.zoom = (st.zoom * factor).clamp(0.2, 8.0);
            st.zoom = (st.zoom * 100.0).round() / 100.0;
            if let Some(da) = st.da.upgrade() {
                da.queue_draw();
            }
        }
        gtk::glib::Propagation::Stop
    });

    window.connect_destroy(|_| gtk::main_quit());
    window.show_all();

    // Initial capture of whatever was focused before this window appeared.
    capture_focused(&state);

    gtk::main();
}