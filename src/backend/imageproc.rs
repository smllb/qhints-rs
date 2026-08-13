use crate::child::{Child, ChildKind};
use crate::config::ApplicationRule;
use crate::window_system::WindowInfo;

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use x11rb::connection::Connection;

/// Debug: pre-filter BFS components (before text-word culling).
/// Populated by `detect_children`, read by overlay drawing when
/// `dev.show_text_boxes` or `dev.show_bfs_boxes` is enabled.
pub static DEBUG_BFS_COMPONENTS: Mutex<Vec<Child>> = Mutex::new(Vec::new());

/// Set by main.rs before calling `get_children` — gates debug PNG output.
pub static SAVE_DEBUG_IMAGES: AtomicBool = AtomicBool::new(false);

use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
use x11rb::rust_connection::RustConnection;

/// Capture the focused window via X11 and detect UI elements within it.
pub fn get_children(
    window_info: &WindowInfo,
    rule: &ApplicationRule,
) -> Result<Vec<Child>, Box<dyn std::error::Error>> {
    let (x, y, mut w, mut h) = window_info.extents;
    if w <= 0 { w = 1; }
    if h <= 0 { h = 1; }

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

    // X11 returns BGRA; reorder to RGBA for `detect_children`.
    let mut rgba = image::RgbaImage::new(w as u32, h as u32);
    for (i, chunk) in data.chunks_exact(4).enumerate() {
        if i >= (w * h) as usize { break; }
        let cx = (i as u32) % (w as u32);
        let cy = (i as u32) / (w as u32);
        rgba.put_pixel(cx, cy, image::Rgba([chunk[2], chunk[1], chunk[0], 255]));
    }

    detect_children(&image::DynamicImage::ImageRgba8(rgba), rule, x as f64, y as f64)
}

/// Intermediate results from `detect_children_debug`, exposing the pipeline
/// stages (luma, Canny edges, words, pre-filter BFS components) so callers
/// (e.g. the screenshot benchmark) can render debug output.
pub struct DetectionDebug {
    pub children: Vec<Child>,
    pub luma: image::GrayImage,
    pub edges: image::GrayImage,
    pub words: Vec<Child>,
    pub all_bfs: Vec<Child>,
}

/// Detect UI elements (`Text` words + `Element` BFS components) in an image.
///
/// Pure-image pipeline shared by the X11 backend and the screenshot benchmark
/// tests. `origin_x`/`origin_y` are added to absolute positions (screen coords
/// for live capture, `0` for headless images).
pub fn detect_children(
    img: &image::DynamicImage,
    rule: &ApplicationRule,
    origin_x: f64,
    origin_y: f64,
) -> Result<Vec<Child>, Box<dyn std::error::Error>> {
    Ok(detect_children_debug(img, rule, origin_x, origin_y)?.children)
}

/// Like `detect_children`, but also returns the intermediate images used for
/// debug rendering (`luma`, `edges`, `words`, `all_bfs`).
pub fn detect_children_debug(
    img: &image::DynamicImage,
    rule: &ApplicationRule,
    origin_x: f64,
    origin_y: f64,
) -> Result<DetectionDebug, Box<dyn std::error::Error>> {
    let rgba = img.to_rgba8();
    let w = rgba.width();
    let h = rgba.height();

    // 1. Convert to three grayscale versions in one pass:
    //    - luma (weighted luminance) for debug and text projection
    //    - max_img / min_img (max- and min-of-RGB) for edge detection.
    //      max-of-RGB catches dark-on-light edges; min-of-RGB catches bright
    //      colored text (e.g. orange on white) that max-of-RGB is blind to
    //      (both channels max out at 255 there).
    let mut luma = image::GrayImage::new(w, h);
    let mut max_img = image::GrayImage::new(w, h);
    let mut min_img = image::GrayImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let r = p[0] as f32;
        let g = p[1] as f32;
        let b = p[2] as f32;
        let l = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
        let max_val = p[0].max(p[1]).max(p[2]);
        let min_val = p[0].min(p[1]).min(p[2]);
        luma.put_pixel(x, y, image::Luma([l]));
        max_img.put_pixel(x, y, image::Luma([max_val]));
        min_img.put_pixel(x, y, image::Luma([min_val]));
    }

    if SAVE_DEBUG_IMAGES.load(Ordering::Relaxed) {
        let _ = std::fs::create_dir_all("/tmp/qhints_debug");
        let _ = luma.save("/tmp/qhints_debug/01_luma.png");
    }

    // 2. Edge detection on max-of-RGB, optionally ORed with min-of-RGB.
    let scale = rule.detection_scale;
    let w2 = ((w as f64) * scale) as u32;
    let h2 = ((h as f64) * scale) as u32;
    let max_src = if scale > 1.0 {
        image::imageops::resize(&max_img, w2, h2, image::imageops::FilterType::Nearest)
    } else {
        max_img
    };
    let edges_max = imageproc::edges::canny(&max_src, rule.canny_min_val as f32, rule.canny_max_val as f32);
    let mut edges = edges_max;
    if rule.min_channel_edges {
        let min_src = if scale > 1.0 {
            image::imageops::resize(&min_img, w2, h2, image::imageops::FilterType::Nearest)
        } else {
            min_img
        };
        let edges_min = imageproc::edges::canny(&min_src, rule.canny_min_val as f32, rule.canny_max_val as f32);
        for (x, y, p) in edges_min.enumerate_pixels() {
            let e = edges.get_pixel_mut(x, y);
            if p[0] > e[0] {
                e[0] = p[0];
            }
        }
    }

    if SAVE_DEBUG_IMAGES.load(Ordering::Relaxed) {
        let _ = std::fs::create_dir_all("/tmp/qhints_debug");
        let _ = edges.save("/tmp/qhints_debug/02_edges.png");
    }

    // Text detection still uses luminance projection
    let luma_process = if scale > 1.0 {
        image::imageops::resize(&luma, w2, h2, image::imageops::FilterType::Nearest)
    } else {
        luma.clone()
    };

    // 3. Detect text words on upscaled undilated edges — scale back later
    let inv_scale = 1.0 / scale;
    let words_raw = detect_text_words(&edges, &luma_process, w2, h2, 0, 0);
    let words: Vec<Child> = words_raw.into_iter().map(|mut w| {
        w.relative_position.0 *= inv_scale;
        w.relative_position.1 *= inv_scale;
        w.width = (w.width * inv_scale).max(1.0);
        w.height = (w.height * inv_scale).max(1.0);
        w.absolute_position.0 = origin_x + w.relative_position.0;
        w.absolute_position.1 = origin_y + w.relative_position.1;
        w
    }).collect();

    // 4. Dilate edges on upscaled image
    let img_w = w2;
    let img_h = h2;
    let radius = (rule.kernel_size / 2) as u8;
    let dilated = imageproc::morphology::dilate(
        &edges,
        imageproc::distance_transform::Norm::LInf,
        radius,
    );

    // 5. BFS on dilated upscaled edges — scale coordinates back
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
            let rpx = (min_x as f64 * inv_scale).floor();
            let rpy = (min_y as f64 * inv_scale).floor();
            let cw = ((max_x - min_x + 1) as f64) * inv_scale;
            let ch = ((max_y - min_y + 1) as f64) * inv_scale;
            all_components.push(Child {
                absolute_position: (origin_x + rpx, origin_y + rpy),
                relative_position: (rpx, rpy),
                width: cw.ceil(),
                height: ch.ceil(),
                kind: ChildKind::Element,
            });
        }
    }

    // Filter out large components (likely containers, not UI elements)
    let max_container_w = w as f64 * 0.5;
    let max_container_h = h as f64 * 0.5;
    let pre_len = all_components.len();
    all_components.retain(|c| c.width < max_container_w && c.height < max_container_h);
    if all_components.len() < pre_len {
        log::debug!("Filtered {} large container components", pre_len - all_components.len());
    }

    // 6. Save pre-filter components for overlay debug rendering.
    if let Ok(mut debug_bfs) = DEBUG_BFS_COMPONENTS.lock() {
        *debug_bfs = all_components.clone();
    }

    // 7. For each text word box, count how many BFS components overlap it.
    //    If a word box spans 2+ BFS components → keep all as Element but
    //    add the word box as a separate Text child (real multi-character text).
    //    If a word box overlaps only 1 BFS component → 95% threshold decides
    //    whether that component is Text or stays Element.
    let word_rects: Vec<(f64, f64, f64, f64)> = words.iter().map(|c| {
        (c.relative_position.0, c.relative_position.1, c.width, c.height)
    }).collect();

    // For each BFS component, track which word index has the max overlap
    let n_words = word_rects.len();
    let mut bfs_overlap_count = vec![0u32; all_components.len()];
    let mut bfs_best_word = vec![n_words; all_components.len()]; // index n_words = none
    let mut bfs_best_overlap = vec![0.0f64; all_components.len()];
    let mut word_bfs_count = vec![0u32; n_words];
    let mut word_bfs_indices: Vec<Vec<usize>> = vec![Vec::new(); n_words];

    for (bi, comp) in all_components.iter().enumerate() {
        let cx = comp.relative_position.0;
        let cy = comp.relative_position.1;
        let cw = comp.width;
        let ch = comp.height;
        let area = cw * ch;
        if area <= 0.0 { continue; }
        for (wi, &(wx, wy, ww, wh)) in word_rects.iter().enumerate() {
            let ix1 = cx.max(wx);
            let iy1 = cy.max(wy);
            let ix2 = (cx + cw).min(wx + ww);
            let iy2 = (cy + ch).min(wy + wh);
            if ix1 < ix2 && iy1 < iy2 {
                let overlap = (ix2 - ix1) * (iy2 - iy1) / area;
                bfs_overlap_count[bi] += 1;
                word_bfs_count[wi] += 1;
                word_bfs_indices[wi].push(bi);
                if overlap > bfs_best_overlap[bi] {
                    bfs_best_overlap[bi] = overlap;
                    bfs_best_word[bi] = wi;
                }
            }
        }
    }

    let all_bfs = all_components.clone();

    let mut children: Vec<Child> = Vec::with_capacity(all_components.len() + words.len());

    for (bi, mut comp) in all_components.into_iter().enumerate() {
        if bfs_best_overlap[bi] > 0.95 {
            comp.kind = ChildKind::Text;
        }
        children.push(comp);
    }

    // Add multi-BFS text word boxes as separate Text children
    let mut added_words = 0u32;
    for (wi, word) in words.iter().enumerate() {
        if word_bfs_count[wi] >= 2 {
            children.push(word.clone());
            added_words += 1;
        }
    }
    log::debug!("  word_bfs_counts: {:?}, added_words: {}", word_bfs_count, added_words);

    // Debug images
    if SAVE_DEBUG_IMAGES.load(Ordering::Relaxed) {
        if let Ok(bfs) = DEBUG_BFS_COMPONENTS.lock() {
            if !bfs.is_empty() {
                let _ = draw_boxes(&luma, &words, &bfs, &children,
                    "/tmp/qhints_debug/04_bfs_debug.png");
            }
        }
    }

    log::debug!("imageproc: {} BFS components ({} text, {} element)",
        children.len(),
        children.iter().filter(|c| c.kind == ChildKind::Text).count(),
        children.iter().filter(|c| c.kind == ChildKind::Element).count());

    Ok(DetectionDebug {
        children,
        luma,
        edges,
        words,
        all_bfs,
    })
}

/// Draw debug boxes (text=blue, all BFS=red, kept=green) on the luma image.
pub fn draw_boxes(
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
