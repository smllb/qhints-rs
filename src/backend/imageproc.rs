use crate::child::{Child, ChildKind};
use crate::config::ApplicationRule;
use crate::window_system::WindowInfo;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
use x11rb::rust_connection::RustConnection;

pub fn get_children(
    window_info: &WindowInfo,
    rule: &ApplicationRule,
) -> Result<Vec<Child>, Box<dyn std::error::Error>> {
    // 1. Take screenshot
    let (x, y, mut w, mut h) = window_info.extents;
    if w <= 0 { w = 1; }
    if h <= 0 { h = 1; }

    // Small delay to let UI settle
    

    let (conn, screen_num) = RustConnection::connect(None)?;
    let setup = conn.setup();
    let root = setup.roots[screen_num].root;

    let reply = conn.get_image(
        ImageFormat::Z_PIXMAP,
        root,
        x as i16, y as i16,
        w as u16, h as u16,
        !0,
    )?.reply()?;
    let data = reply.data;

    if data.len() < (w * h * 4) as usize {
        return Err("Image data too short".into());
    }

    // 2. Convert BGRA to Luma8
    let mut luma = image::GrayImage::new(w as u32, h as u32);
    for (i, chunk) in data.chunks_exact(4).enumerate() {
        if i >= (w * h) as usize { break; }
        let b = chunk[0] as f32;
        let g = chunk[1] as f32;
        let r = chunk[2] as f32;
        let l = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
        let cx = (i as u32) % (w as u32);
        let cy = (i as u32) / (w as u32);
        luma.put_pixel(cx, cy, image::Luma([l]));
    }

    // 3. Edge detection
    let edges = imageproc::edges::canny(
        &luma,
        rule.canny_min_val as f32,
        rule.canny_max_val as f32,
    );

    // Debug dump
    let _ = std::fs::create_dir_all("/tmp/qhints_debug");
    let _ = luma.save("/tmp/qhints_debug/01_luma.png");
    let _ = edges.save("/tmp/qhints_debug/02_edges.png");

    // 4. Detect text first — runs on undilated edges for precise word boxes.
    //    Returns (words, line_bands).  We keep line_bands for later debug use.
    let (words, _line_bands) = detect_text_words(&edges, &luma, w as u32, h as u32, x, y);

    // 5. Mask out text word regions from edge image so BFS won't find text
    //    artifacts.  Dilate the remaining edges so nearby icon edges merge
    //    into cohesive components.
    let img_w = w as u32;
    let img_h = h as u32;
    let mut icon_edges = edges.clone();
    for word in &words {
        // Shrink mask by 3px on each side so nearby icon edges survive
        let margin = 3u32;
        let wx = (word.relative_position.0 as u32).saturating_add(margin);
        let wy = (word.relative_position.1 as u32).saturating_add(margin);
        let ww = (word.width as u32).saturating_sub(margin * 2);
        let wh = (word.height as u32).saturating_sub(margin * 2);
        if ww == 0 || wh == 0 { continue; }
        let ex = wx.saturating_add(ww).min(img_w);
        let ey = wy.saturating_add(wh).min(img_h);
        for y in wy..ey {
            for x in wx..ex {
                icon_edges.put_pixel(x, y, image::Luma([0]));
            }
        }
    }

    let radius = (rule.kernel_size / 2) as u8;
    let dilated = imageproc::morphology::dilate(
        &icon_edges,
        imageproc::distance_transform::Norm::LInf,
        radius,
    );

    // 6. BFS connected components on dilated icon edges
    let mut visited = vec![false; (img_w * img_h) as usize];
    let mut children: Vec<Child> = Vec::new();

    for start_y in 0..img_h {
        for start_x in 0..img_w {
            let idx = (start_y * img_w + start_x) as usize;
            if visited[idx] || dilated.get_pixel(start_x, start_y)[0] == 0 {
                continue;
            }

            // BFS
            let mut min_x = start_x;
            let mut min_y = start_y;
            let mut max_x = start_x;
            let mut max_y = start_y;
            let mut queue = std::collections::VecDeque::new();
            queue.push_back((start_x, start_y));
            visited[idx] = true;

            while let Some((cx, cy)) = queue.pop_front() {
                if cx < min_x { min_x = cx; }
                if cy < min_y { min_y = cy; }
                if cx > max_x { max_x = cx; }
                if cy > max_y { max_y = cy; }

                let neighbors: [(i64, i64); 4] = [
                    (cx as i64 - 1, cy as i64),
                    (cx as i64 + 1, cy as i64),
                    (cx as i64, cy as i64 - 1),
                    (cx as i64, cy as i64 + 1),
                ];

                for (nx, ny) in neighbors {
                    if nx < 0 || ny < 0 || nx >= img_w as i64 || ny >= img_h as i64 {
                        continue;
                    }
                    let nidx = (ny as u32 * img_w + nx as u32) as usize;
                    if !visited[nidx] && dilated.get_pixel(nx as u32, ny as u32)[0] > 0 {
                        visited[nidx] = true;
                        queue.push_back((nx as u32, ny as u32));
                    }
                }
            }

            children.push(Child {
                absolute_position: (
                    (x + min_x as i32) as f64,
                    (y + min_y as i32) as f64,
                ),
                relative_position: (min_x as f64, min_y as f64),
                width: (max_x - min_x + 1) as f64,
                height: (max_y - min_y + 1) as f64,
                kind: ChildKind::Element,
            });
        }
    }

    log::debug!("imageproc: {} BFS icon components, {} text words", children.len(), words.len());

    children.extend(words);
    Ok(children)
}

/// Detect text lines in the edge image via horizontal projection, then split
/// each line into word segments via vertical projection.
///
/// Returns word-level `Child` rects in screen coordinates, and the detected
/// text line bands (y0, y1) used to determine which BFS components sit
/// inside text regions.
fn detect_text_words(
    edges: &image::GrayImage,
    _luma: &image::GrayImage,
    img_w: u32,
    img_h: u32,
    win_x: i32,
    win_y: i32,
) -> (Vec<Child>, Vec<(u32, u32)>) {
    if img_w == 0 || img_h == 0 {
        return (Vec::new(), Vec::new());
    }

    // ── Step 1: horizontal projection — edges per row ─────────────────────
    let mut row_sums = vec![0u32; img_h as usize];
    for y in 0..img_h {
        let mut sum = 0u32;
        for x in 0..img_w {
            if edges.get_pixel(x, y)[0] > 0 {
                sum += 1;
            }
        }
        row_sums[y as usize] = sum;
    }

    // Threshold: a row is "text" if it has at least 0.5 % edge pixels
    let row_threshold = (img_w as f32 * 0.005).max(3.0) as u32;
    let min_line_height = 8u32;
    let max_gap = (img_h as f32 * 0.02).max(2.0) as u32; // merge lines separated by ≤2% of height

    // ── Step 2: find text line bands ──────────────────────────────────────
    let mut line_bands: Vec<(u32, u32)> = Vec::new();
    let mut in_line = false;
    let mut band_start = 0u32;
    let mut gap_after = 0u32;

    for y in 0..img_h {
        if row_sums[y as usize] > row_threshold {
            if !in_line {
                band_start = y;
                in_line = true;
                gap_after = 0;
            } else {
                gap_after = 0;
            }
        } else if in_line {
            gap_after += 1;
            if gap_after > max_gap {
                let line_h = y - gap_after - band_start;
                if line_h >= min_line_height {
                    line_bands.push((band_start, y - gap_after));
                }
                in_line = false;
            }
        }
    }
    if in_line {
        let line_h = img_h - band_start;
        if line_h >= min_line_height {
            line_bands.push((band_start, img_h));
        }
    }

    if line_bands.is_empty() {
        return (Vec::new(), line_bands);
    }

    // ── Step 3: vertical projection per line band → word segments ─────────
    let mut word_rects: Vec<(u32, u32, u32, u32)> = Vec::new();
    let gap_ratio = 0.25; // column must have <25% of line-height edge pixels to be a gap
    let min_word_width = 4u32;
    let min_space_width = 3u32; // require 3+ consecutive gap columns to split

    for &(ly0, ly1) in &line_bands {
        let line_h = ly1 - ly0;
        let col_gap_threshold = (line_h as f32 * gap_ratio).max(2.0) as u32;

        // Column sums within this line band
        let mut col_sums = vec![0u32; img_w as usize];
        for y in ly0..ly1 {
            for x in 0..img_w {
                if edges.get_pixel(x, y)[0] > 0 {
                    col_sums[x as usize] += 1;
                }
            }
        }

        // Find word segments — only split at gaps ≥ min_space_width columns
        let mut in_word = false;
        let mut word_start = 0u32;
        let mut gap_run = 0u32;

        for x in 0..img_w {
            if col_sums[x as usize] > col_gap_threshold {
                gap_run = 0;
                if !in_word {
                    word_start = x;
                    in_word = true;
                }
            } else if in_word {
                gap_run += 1;
                if gap_run >= min_space_width {
                    let word_w = x - gap_run + 1 - word_start;
                    if word_w >= min_word_width {
                        word_rects.push((word_start, ly0, word_w, line_h));
                    }
                    in_word = false;
                    gap_run = 0;
                }
            }
        }
        if in_word {
            let word_w = img_w - word_start;
            if word_w >= min_word_width {
                word_rects.push((word_start, ly0, word_w, line_h));
            }
        }
    }

    // Convert to Child elements
    let children: Vec<Child> = word_rects
        .into_iter()
        .map(|(wx, wy, ww, wh)| Child {
            absolute_position: (
                (win_x + wx as i32) as f64,
                (win_y + wy as i32) as f64,
            ),
            relative_position: (wx as f64, wy as f64),
            width: ww as f64,
            height: wh as f64,
            kind: ChildKind::Text,
        })
        .collect();
    (children, line_bands)
}

