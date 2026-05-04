use crate::child::Child;
use crate::config::Config;
use gtk::cairo;
use gtk::cairo::Context;
use std::collections::HashMap;

/// Draw all visible hints onto the cairo context.
pub fn draw_hints(
    cr: &Context,
    config: &Config,
    hints: &HashMap<String, usize>,
    children: &[Child],
    typed: &str,
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

    // Pre-compute bounding boxes centered on child elements
    let mut hint_rects: Vec<(String, usize, f64, f64, f64, f64)> = Vec::new();

    for (label, &child_idx) in hints {
        let child = &children[child_idx];
        let (rx, ry) = child.relative_position;

        let extents = cr.text_extents(label).unwrap();
        let w = extents.width() + h.hint_width_padding;
        let rect_h = h.hint_height;

        // Center hint on child (like Python version)
        let hx = rx + child.width / 2.0 - w / 2.0;
        let hy = ry + child.height / 2.0 - rect_h / 2.0;

        hint_rects.push((label.clone(), child_idx, hx, hy, w, rect_h));
    }

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

    // Draw only kept hints
    for (idx, item) in visible.iter().enumerate() {
        if !kept[idx] {
            continue;
        }

        let (ref label, _, hx, hy, w, rect_h) = **item;

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
        cr.set_source_rgba(h.hint_border_r, h.hint_border_g, h.hint_border_b, h.hint_border_a);
        cr.set_line_width(h.hint_border_width);
        let _ = cr.stroke();

        // Per-character text rendering
        let mut text_x = hx + h.hint_width_padding / 2.0;
        let text_y = hy + rect_h * 0.75;

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