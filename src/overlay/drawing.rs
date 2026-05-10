use crate::backend::imageproc::DEBUG_BFS_COMPONENTS;
use crate::child::{Child, ChildKind};
use crate::config::Config;
use gtk::cairo;
use gtk::cairo::Context;
use std::collections::HashMap;

/// Draw all visible hints onto the cairo context.
use crate::overlay::ActiveHook;

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
) {
    let h = &config.hints;

    // Clear background (fully transparent)
    cr.set_operator(cairo::Operator::Source);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    let _ = cr.paint();
    cr.set_operator(cairo::Operator::Over);

    // Select font
    cr.select_font_face(
        &h.hint_font_face,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    cr.set_font_size(h.hint_font_size);

    // In advanced mode with both hooks placed, hide hints — only markers and spotlight shown
    let hide_all_hints = (advanced_mode && selection_end_child.is_some())
        || (drag_advanced_mode && drag_dest_child.is_some());

    // Pre-compute bounding boxes centered on child elements
    let mut hint_rects: Vec<(String, usize, f64, f64, f64, f64)> = Vec::new();

    if !hide_all_hints {
    for (label, &child_idx) in hints {
        if consumed_hints.contains(&child_idx) {
            continue;
        }

        let child = &children[child_idx];
        let (rx, ry) = child.relative_position;

        let extents = cr.text_extents(label).unwrap();
        let w = extents.width() + h.hint_width_padding;
        let rect_h = h.hint_height;

        // Center hint on child
        let hx = rx + child.width / 2.0 - w / 2.0;
        let hy = ry + child.height / 2.0 - rect_h / 2.0;

        hint_rects.push((label.clone(), child_idx, hx, hy, w, rect_h));
    }

    // Pre-compute font ascent for vertical centering of text in hint boxes
    let font_ext = cr.font_extents().unwrap();
    let font_ascent = font_ext.ascent();
    let font_descent = font_ext.descent();

    // Filter to hints matching the typed prefix
    let visible: Vec<&(String, usize, f64, f64, f64, f64)> = hint_rects
        .iter()
        .filter(|(label, _, _, _, _, _)| label.starts_with(typed))
        .collect();

    if visible.is_empty() {
        return;
    }

    // Overlap culling using configurable threshold
    // hint_overlap_threshold: 0 = show all, 100 = very aggressive
    let overlap_limit = if h.hint_overlap_threshold == 0.0 {
        f64::MAX
    } else {
        (100.0 - h.hint_overlap_threshold) / 100.0
    };

    let mut kept = vec![true; visible.len()];

    for i in 0..visible.len() {
        if !kept[i] {
            continue;
        }
        let (_, _, x1, y1, w1, h1) = visible[i];
        let r1 = (*x1, *y1, x1 + w1, y1 + h1);

        for j in (i + 1)..visible.len() {
            if !kept[j] {
                continue;
            }
            let (_, _, x2, y2, w2, h2) = visible[j];
            let r2 = (*x2, *y2, x2 + w2, y2 + h2);

            if overlap_fraction(r1, r2) > overlap_limit {
                // Deterministic: keep the one that comes first (top-left)
                kept[j] = false;
            }
        }
    }

    // Spotlight: dark overlay with soft radial holes around kept hints
    if config.dev.spotlight && !typed.is_empty() {
        let (ww, wh) = window_size;
        let opacity = config.dev.spotlight_opacity;
        let radius_mul = config.dev.spotlight_radius;

        cr.set_source_rgba(0.0, 0.0, 0.0, opacity);
        cr.rectangle(0.0, 0.0, ww, wh);
        let _ = cr.fill();

        for (idx, item) in visible.iter().enumerate() {
            if !kept[idx] { continue; }
            let (_, _, hx, hy, w, rect_h) = **item;
            let cx = hx + w / 2.0;
            let cy = hy + rect_h / 2.0;
            let inner_r = (w.max(rect_h) / 2.0) * 0.8;
            let outer_r = (w.max(rect_h) / 2.0) * radius_mul;

            let grad = cairo::RadialGradient::new(cx, cy, inner_r, cx, cy, outer_r);
            grad.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 1.0);
            grad.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
            cr.set_source(&grad).unwrap();
            cr.set_operator(cairo::Operator::DestOut);
            cr.arc(cx, cy, outer_r, 0.0, 2.0 * std::f64::consts::PI);
            let _ = cr.fill();
        }
        cr.set_operator(cairo::Operator::Over);
    }

    // Draw only kept hints
    for (idx, item) in visible.iter().enumerate() {
        if !kept[idx] {
            continue;
        }

        let (ref label, child_idx, hx, hy, w, rect_h) = **item;

        // Shadow
        if h.hint_shadow {
            draw_rounded_rect(
                cr,
                hx + h.hint_shadow_offset_x,
                hy + h.hint_shadow_offset_y,
                w,
                rect_h,
                h.hint_corner_radius,
            );
            cr.set_source_rgba(h.hint_shadow_r, h.hint_shadow_g, h.hint_shadow_b, h.hint_shadow_a);
            let _ = cr.fill();
        }

        // Background
        draw_rounded_rect(cr, hx, hy, w, rect_h, h.hint_corner_radius);
        cr.set_source_rgba(
            h.hint_background_r,
            h.hint_background_g,
            h.hint_background_b,
            h.hint_background_a,
        );
        let _ = cr.fill_preserve();

        // Border
        if text_selection_mode && children[child_idx].kind == ChildKind::Text {
            cr.set_source_rgba(h.text_select_border_r, h.text_select_border_g, h.text_select_border_b, h.text_select_border_a);
            cr.set_line_width(h.hint_border_width + 1.5 + if advanced_mode { h.advanced_border_extra_width } else { 0.0 });
        } else if double_click_mode {
            cr.set_source_rgba(h.hint_border_r, h.hint_border_g, h.hint_border_b, h.hint_border_a);
            cr.set_line_width(h.hint_border_width + 2.0);
        } else if drag_mode {
            cr.set_source_rgba(0.2, 0.8, 0.2, 0.9);
            cr.set_line_width(h.hint_border_width + 1.5 + if drag_advanced_mode { h.advanced_border_extra_width } else { 0.0 });
        } else {
            cr.set_source_rgba(h.hint_border_r, h.hint_border_g, h.hint_border_b, h.hint_border_a);
            cr.set_line_width(h.hint_border_width);
        }
        let _ = cr.stroke();

        // Per-character text rendering
        let mut text_x = hx + h.hint_width_padding / 2.0;
        let text_y = hy + (rect_h + font_ascent - font_descent) / 2.0;

        for (ci, ch) in label.chars().enumerate() {
            let display_ch = if h.hint_upercase {
                ch.to_uppercase().next().unwrap_or(ch)
            } else {
                ch
            };

            let ch_str = display_ch.to_string();

            if ci < typed.len() {
                cr.set_source_rgba(
                    h.hint_pressed_font_r,
                    h.hint_pressed_font_g,
                    h.hint_pressed_font_b,
                    h.hint_pressed_font_a,
                );
            } else if ci == 0 {
                cr.set_font_size(h.hint_font_size + h.hint_first_font_size_boost);
                cr.set_source_rgba(
                    h.hint_first_font_r,
                    h.hint_first_font_g,
                    h.hint_first_font_b,
                    h.hint_first_font_a,
                );
            } else {
                cr.set_font_size(h.hint_font_size);
                cr.set_source_rgba(h.hint_font_r, h.hint_font_g, h.hint_font_b, h.hint_font_a);
            }

            cr.move_to(text_x, text_y);
            let _ = cr.show_text(&ch_str);

            let char_ext = cr.text_extents(&ch_str).unwrap();
            text_x += char_ext.x_advance();

            if ci == 0 {
                cr.set_font_size(h.hint_font_size);
            }
        }
    }
    } // end if !hide_all_hints

    // ── Spotlight rectangle between hooks (text selection or drag) ────
    let spotlight = if advanced_mode {
        (selection_start_child, selection_end_child, config.dev.advanced_spotlight_opacity)
    } else {
        (None, None, 0.0)
    };
    if let (Some(si), Some(ei)) = (spotlight.0, spotlight.1) {
        if si < children.len() && ei < children.len() {
            let sc = &children[si];
            let ec = &children[ei];
            let marker_x = |child: &Child, is_end: bool, off_x: f64| {
                let base = match child.kind {
                    ChildKind::Text => {
                        if is_end { child.relative_position.0 + child.width }
                        else { child.relative_position.0 }
                    }
                    ChildKind::Element => child.relative_position.0 + child.width / 2.0,
                };
                base + off_x * child.width.max(child.height)
            };
            let (sx, sy, ex, ey) = if advanced_mode {
                let sx = marker_x(sc, false, selection_start_offset_x);
                let sy = sc.relative_position.1 + selection_start_offset_y * sc.height;
                let ex = marker_x(ec, true, selection_end_offset_x);
                let ey = ec.relative_position.1 + ec.height + selection_end_offset_y * ec.height;
                (sx, sy, ex, ey)
            } else {
                let sx = sc.relative_position.0 + sc.width / 2.0 + drag_source_offset_x * sc.width;
                let sy = sc.relative_position.1 + sc.height / 2.0 + drag_source_offset_y * sc.height;
                let ex = ec.relative_position.0 + ec.width / 2.0 + drag_dest_offset_x * ec.width;
                let ey = ec.relative_position.1 + ec.height / 2.0 + drag_dest_offset_y * ec.height;
                (sx, sy, ex, ey)
            };
            let (x1, y1, x2, y2) = (sx.min(ex), sy.min(ey), sx.max(ex), sy.max(ey));
            let op = spotlight.2;
            if op > 0.0 {
                let (ww, wh) = window_size;
                cr.set_source_rgba(0.0, 0.0, 0.0, op);
                cr.rectangle(0.0, 0.0, ww, wh);
                let _ = cr.fill();
                cr.set_operator(cairo::Operator::DestOut);
                cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
                cr.rectangle(x1.max(0.0), y1.max(0.0), (x2 - x1).max(1.0), (y2 - y1).max(1.0));
                let _ = cr.fill();
                cr.set_operator(cairo::Operator::Over);
            }
        }
    }

    // ── Hooks at selection markers ─────────────────────────────────────
    // Pulse animation — always active when any marker is visible
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64;
    let period = (config.hints.text_select_pulse_period_ms as f64).max(1.0);
    let freq = std::f64::consts::TAU / period;
    let pulse = ((t * freq).sin() + 1.0) * 0.5; // 0..1 sine wave

    let draw_marker = |cr: &Context, child: &Child, off_x: f64, off_y: f64, r: f64, g: f64, b: f64, is_end: bool, active: bool| {
        let px0 = match child.kind {
            ChildKind::Text => {
                if is_end { child.relative_position.0 + child.width - 2.0 }
                else { child.relative_position.0 - 2.0 }
            }
            ChildKind::Element => child.relative_position.0 + child.width / 2.0 - 2.0,
        };
        let px = px0 + off_x * child.width;
        let py = child.relative_position.1 + off_y * child.height;
        let ph = child.height;
        let (alpha, lw) = if active {
            (0.5 + pulse * 0.5, 1.5 + pulse * 4.0)
        } else {
            (0.6 + pulse * 0.2, 2.5 + pulse * 1.0)
        };
        cr.set_source_rgba(r, g, b, alpha);
        cr.set_line_width(lw);
        cr.move_to(px, py);
        cr.line_to(px, py + ph);
        let _ = cr.stroke();
        // Bright flash on newly placed or tabbed marker
        if pulse_bright_remaining > 0 {
            let max_ticks = marker_bright_duration_ticks.max(1) as f64;
            let flash = (pulse_bright_remaining as f64) / max_ticks;
            cr.set_source_rgba(r, g, b, flash * 0.6);
            cr.set_line_width(lw + flash * 4.0);
            cr.move_to(px, py);
            cr.line_to(px, py + ph);
            let _ = cr.stroke();
        }
    };

    // Text selection markers (start)
    if let Some(start_idx) = selection_start_child {
        if start_idx < children.len() {
            let active = advanced_mode && active_hook == ActiveHook::Start;
            draw_marker(cr, &children[start_idx], selection_start_offset_x, selection_start_offset_y, 0.9, 0.1, 0.1, false, active);
        }
    }
    // Text selection markers (end)
    if advanced_mode {
        if let Some(end_idx) = selection_end_child {
            if end_idx < children.len() {
                let active = active_hook == ActiveHook::End;
                draw_marker(cr, &children[end_idx], selection_end_offset_x, selection_end_offset_y, 1.0, 0.6, 0.0, true, active);
            }
        }
    }
    // Drag markers
    if drag_mode {
        let draw_dot = |cr: &Context, cx: f64, cy: f64, rad: f64, r: f64, g: f64, b: f64, a: f64| {
                cr.set_source_rgba(r, g, b, a);
                if drag_marker_square {
                    let s = rad * 1.5;
                    cr.rectangle(cx - s, cy - s, s * 2.0, s * 2.0);
                } else {
                    cr.arc(cx, cy, rad, 0.0, 2.0 * std::f64::consts::PI);
                }
                let _ = cr.fill();
        };
        // Source marker
        if let Some((sx, sy)) = drag_source_pos {
            let active = drag_advanced_mode && active_hook == ActiveHook::Start;
            let dim = drag_source_size.0.max(drag_source_size.1).max(1.0);
            let ox = window_origin.0 as f64;
            let oy = window_origin.1 as f64;
            let px = sx - ox + drag_source_offset_x * dim;
            let py = sy - oy + drag_source_offset_y * dim;
            let alpha = if active { 0.5 + pulse * 0.5 } else { 0.6 + pulse * 0.2 };
            let r = drag_marker_size + pulse * 1.5;
            draw_dot(cr, px, py, r, 0.9, 0.1, 0.1, alpha);
            if pulse_bright_remaining > 0 {
                let max_ticks_d = marker_bright_duration_ticks.max(1) as f64;
                let flash = (pulse_bright_remaining as f64) / max_ticks_d;
                draw_dot(cr, px, py, r + flash * 3.0, 1.0, 0.4, 0.4, flash * 0.5);
            }
        }
        // Destination marker
        if let Some(dst_idx) = drag_dest_child {
            if dst_idx < children.len() {
                let child = &children[dst_idx];
                let dim = child.width.max(child.height);
                let px = child.relative_position.0 + child.width / 2.0 - 2.0 + drag_dest_offset_x * dim;
                let py = child.relative_position.1 + child.height / 2.0 + drag_dest_offset_y * dim;
                let active = drag_advanced_mode && active_hook == ActiveHook::End;
                let alpha = if active { 0.5 + pulse * 0.5 } else { 0.6 + pulse * 0.2 };
                let r = drag_marker_size + pulse * 1.5;
                draw_dot(cr, px, py, r, 0.2, 0.8, 0.2, alpha);
                if pulse_bright_remaining > 0 {
                    let max_ticks_d = marker_bright_duration_ticks.max(1) as f64;
                    let flash = (pulse_bright_remaining as f64) / max_ticks_d;
                    draw_dot(cr, px, py, r + flash * 3.0, 0.4, 1.0, 0.4, flash * 0.5);
                }
            }
        }
    }

    // ── Text selection mode: show bounding boxes around hinted children ──
    if text_selection_mode && text_selection_show_boxes && !hide_all_hints {
        for &child_idx in hints.values() {
            let child = &children[child_idx];
            cr.rectangle(child.relative_position.0, child.relative_position.1, child.width, child.height);
            cr.set_source_rgba(h.text_select_border_r, h.text_select_border_g, h.text_select_border_b, 0.15);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(h.text_select_border_r, h.text_select_border_g, h.text_select_border_b, 0.6);
            cr.set_line_width(1.5);
            let _ = cr.stroke();
        }
    }

    // ── Drag mode: show bounding boxes around hinted children ─────────
    if drag_mode && drag_show_boxes && !hide_all_hints {
        for &child_idx in hints.values() {
            let child = &children[child_idx];
            cr.rectangle(child.relative_position.0, child.relative_position.1, child.width, child.height);
            cr.set_source_rgba(0.2, 0.8, 0.2, 0.15);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(0.2, 0.8, 0.2, 0.6);
            cr.set_line_width(1.5);
            let _ = cr.stroke();
        }
    }

    // ── Dev: pre-filter BFS components (red) ──────────────────────────
    if show_bfs_boxes && !hide_all_hints {
        if let Ok(bfs) = DEBUG_BFS_COMPONENTS.lock() {
            for c in bfs.iter() {
                cr.rectangle(c.relative_position.0, c.relative_position.1, c.width, c.height);
                cr.set_source_rgba(0.9, 0.1, 0.1, 0.08);
                let _ = cr.fill_preserve();
                cr.set_source_rgba(0.9, 0.1, 0.1, 0.5);
                cr.set_line_width(1.5);
                let _ = cr.stroke();
            }
        }
    }

    // ── Dev: text word bounding boxes (blue) ──────────────────────────
    if show_text_boxes && !hide_all_hints {
        for child in children {
            if child.kind == ChildKind::Text {
                cr.rectangle(child.relative_position.0, child.relative_position.1, child.width, child.height);
                cr.set_source_rgba(0.0, 0.6, 1.0, 0.12);
                let _ = cr.fill_preserve();
                cr.set_source_rgba(0.0, 0.3, 0.8, 0.5);
                cr.set_line_width(1.5);
                let _ = cr.stroke();
            }
        }
    }

    // ── Dev: show grid zone boundaries ──────────────────────────────────
    if config.dev.show_grid {
        let (w, h) = window_size;
        let rows = config.first_key_zones.len();
        if rows > 0 {
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.4);
            cr.set_line_width(1.0);

            // Horizontal lines
            for r in 1..rows {
                let y = (r as f64 / rows as f64) * h;
                cr.move_to(0.0, y);
                cr.line_to(w, y);
            }

            // Vertical lines per row
            for r in 0..rows {
                let ncols = config.first_key_zones[r].len();
                for c in 1..ncols {
                    let x = (c as f64 / ncols as f64) * w;
                    let y0 = (r as f64 / rows as f64) * h;
                    let y1 = ((r + 1) as f64 / rows as f64) * h;
                    cr.move_to(x, y0);
                    cr.line_to(x, y1);
                }
            }
            let _ = cr.stroke();
        }
    }
}

/// Draw a rounded rectangle path.
fn draw_rounded_rect(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + r, y + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
    cr.close_path();
}

/// Overlap fraction between two rectangles (as fraction of the smaller area).
fn overlap_fraction(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let ix1 = a.0.max(b.0);
    let iy1 = a.1.max(b.1);
    let ix2 = a.2.min(b.2);
    let iy2 = a.3.min(b.3);

    if ix1 >= ix2 || iy1 >= iy2 {
        return 0.0;
    }

    let intersection = (ix2 - ix1) * (iy2 - iy1);
    let area_a = (a.2 - a.0) * (a.3 - a.1);
    let area_b = (b.2 - b.0) * (b.3 - b.1);
    let min_area = area_a.min(area_b);

    if min_area <= 0.0 {
        0.0
    } else {
        intersection / min_area
    }
}