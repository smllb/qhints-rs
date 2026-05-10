use crate::child::{Child, ChildKind};
use crate::config::ApplicationRule;
use crate::window_system::WindowInfo;

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use x11rb::connection::Connection;

/// Debug: pre-filter BFS components (before text-word culling).
/// Populated by `get_children`, read by overlay drawing when
/// `dev.show_text_boxes` or `dev.show_bfs_boxes` is enabled.
pub static DEBUG_BFS_COMPONENTS: Mutex<Vec<Child>> = Mutex::new(Vec::new());

/// Set by main.rs before calling `get_children` — gates debug PNG output.
pub static SAVE_DEBUG_IMAGES: AtomicBool = AtomicBool::new(false);
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

    // 4. Detect text words on undilated edges
    let words = detect_text_words(&edges, &luma, w as u32, h as u32, x, y);

    // 5. Dilate edges and BFS to find all components (text + icons).
    let img_w = w as u32;
    let img_h = h as u32;
    let radius = (rule.kernel_size / 2) as u8;
    let dilated = imageproc::morphology::dilate(
        &edges,
        imageproc::distance_transform::Norm::LInf,
        radius,
    );

    // 6. BFS on dilated edges
    let mut visited = vec![false; (img_w * img_h) as usize];
    let mut all_components: Vec<Child> = Vec::new();

    for start_y in 0..img_h {
        for start_x in 0..img_w {
            let idx = (start_y * img_w + start_x) as usize;
            if visited[idx] || dilated.get_pixel(start_x, start_y)[0] == 0 {
                continue;
            }
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
            all_components.push(Child {
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

    // 7. Save pre-filter components for overlay debug rendering.
    if let Ok(mut debug_bfs) = DEBUG_BFS_COMPONENTS.lock() {
        *debug_bfs = all_components.clone();
    }

    // 8. Shrink text word boxes by 4px so they don't include nearby icon
    //    edges.  Then cull BFS components whose center falls inside any
    //    shrunk word box — those are text artifacts (character fragments).
    //    Icons adjacent to text survive because their center is outside.
    let margin = 4i32;
    let word_cores: Vec<(f64, f64, f64, f64)> = words.iter().map(|c| {
        let wx = c.relative_position.0 + margin as f64;
        let wy = c.relative_position.1 + margin as f64;
        let ww = (c.width as i32 - margin * 2).max(1) as f64;
        let wh = (c.height as i32 - margin * 2).max(1) as f64;
        (wx, wy, ww, wh)
    }).collect();
    let word_cores_ref = &word_cores;
    let children: Vec<Child> = all_components.into_iter().filter(|comp| {
        let center_x = comp.relative_position.0 + comp.width / 2.0;
        let center_y = comp.relative_position.1 + comp.height / 2.0;
        let inside = word_cores_ref.iter().any(|&(wx, wy, ww, wh)| {
            center_x >= wx && center_x <= wx + ww
                && center_y >= wy && center_y <= wy + wh
        });
        !inside // keep if center is outside all shrunk word boxes
    }).collect();

    // Debug images
    if SAVE_DEBUG_IMAGES.load(Ordering::Relaxed) {
        if let Ok(bfs) = DEBUG_BFS_COMPONENTS.lock() {
            if !bfs.is_empty() {
                let _ = draw_boxes(&luma, &words, &bfs, &children,
                    "/tmp/qhints_debug/04_bfs_debug.png");
            }
        }
    }

    log::debug!("imageproc: {} BFS components, {} text words", children.len(), words.len());

    Ok([children, words].concat())
}

/// Draw debug boxes (text=blue, all BFS=red, kept=green) on the luma image.
fn draw_boxes(
    luma: &image::GrayImage,
    words: &[Child],
    all_bfs: &[Child],
    kept: &[Child],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut img = image::RgbaImage::from_fn(luma.width(), luma.height(), |x, y| {
        let l = luma.get_pixel(x, y)[0];
        image::Rgba([l, l, l, 255])
    });
    // Text word boxes: blue border
    for w in words {
        let (x0, y0) = (w.relative_position.0 as u32, w.relative_position.1 as u32);
        let (x1, y1) = ((x0 + w.width as u32).saturating_sub(1), (y0 + w.height as u32).saturating_sub(1));
        for x in x0..=x1.min(img.width() - 1) {
            img.put_pixel(x, y0, image::Rgba([0, 120, 255, 255]));
            img.put_pixel(x, y1, image::Rgba([0, 120, 255, 255]));
        }
        for y in y0..=y1.min(img.height() - 1) {
            img.put_pixel(x0, y, image::Rgba([0, 120, 255, 255]));
            img.put_pixel(x1, y, image::Rgba([0, 120, 255, 255]));
        }
    }
    // All pre-filter BFS components: red border
    for c in all_bfs {
        let (x0, y0) = (c.relative_position.0 as u32, c.relative_position.1 as u32);
        let (x1, y1) = ((x0 + c.width as u32).saturating_sub(1), (y0 + c.height as u32).saturating_sub(1));
        for x in x0..=x1.min(img.width() - 1) {
            img.put_pixel(x, y0, image::Rgba([255, 0, 0, 200]));
            img.put_pixel(x, y1, image::Rgba([255, 0, 0, 200]));
        }
        for y in y0..=y1.min(img.height() - 1) {
            img.put_pixel(x0, y, image::Rgba([255, 0, 0, 200]));
            img.put_pixel(x1, y, image::Rgba([255, 0, 0, 200]));
        }
    }
    // Kept BFS components (post-filter): green border
    for c in kept {
        let (x0, y0) = (c.relative_position.0 as u32, c.relative_position.1 as u32);
        let (x1, y1) = ((x0 + c.width as u32).saturating_sub(1), (y0 + c.height as u32).saturating_sub(1));
        for x in x0..=x1.min(img.width() - 1) {
            img.put_pixel(x, y0, image::Rgba([0, 200, 0, 200]));
            img.put_pixel(x, y1, image::Rgba([0, 200, 0, 200]));
        }
        for y in y0..=y1.min(img.height() - 1) {
            img.put_pixel(x0, y, image::Rgba([0, 200, 0, 200]));
            img.put_pixel(x1, y, image::Rgba([0, 200, 0, 200]));
        }
    }
    img.save(path)?;
    Ok(())
}

/// Detect text lines via horizontal projection, split each line into word
/// segments via vertical projection. Returns word-level `Child` rects.
fn detect_text_words(
    edges: &image::GrayImage,
    _luma: &image::GrayImage,
    img_w: u32,
    img_h: u32,
    win_x: i32,
    win_y: i32,
) -> Vec<Child> {
    if img_w == 0 || img_h == 0 {
        return Vec::new();
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
        return Vec::new();
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

    word_rects
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
        .collect()
}

