use crate::child::{Child, ChildKind};
use crate::config::ApplicationRule;
use crate::window_system::WindowInfo;

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use x11rb::connection::Connection;
use rayon::prelude::*;

/// Debug: pre-filter BFS components (before text-word culling).
/// Populated by `detect_children`, read by overlay drawing when
/// `dev.show_text_boxes` or `dev.show_bfs_boxes` is enabled.
pub static DEBUG_BFS_COMPONENTS: Mutex<Vec<Child>> = Mutex::new(Vec::new());

/// Set by main.rs before calling `get_children` — gates debug PNG output.
pub static SAVE_DEBUG_IMAGES: AtomicBool = AtomicBool::new(false);

/// Compare-branch A: fused single contrast channel selection.
#[derive(Clone, Copy, PartialEq)]
enum FusedChannel {
    /// max − min/2
    A1,
    /// (luma + max − min)/2
    A2,
}

fn fused_channel_mode() -> FusedChannel {
    match std::env::var("FUSED_CHANNEL").as_deref() {
        Ok("a2") => FusedChannel::A2,
        _ => FusedChannel::A1,
    }
}

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

    // 1. Convert to two grayscale versions in one pass:
    //    - luma (weighted luminance) for debug and text projection
    //    - fused (single contrast channel) for edge detection. Replaces the
    //      max-of-RGB + min-of-RGB double channel with ONE channel that keeps
    //      both dark-on-light edges (max domain) and bright saturated text
    //      edges (min domain, e.g. orange on white). Normalized so the value
    //      stays in u8 range without clamping away the orange-vs-white step.
    //        A1 = max - min/2        (≈ (2·max − min)/2)
    //        A2 = (luma + max − min)/2
    let fused_mode = fused_channel_mode();
    let mut luma = image::GrayImage::new(w, h);
    let mut fused_img = image::GrayImage::new(w, h);
    {
        let rgba_raw = rgba.as_raw();
        let luma_slice: &mut [u8] = &mut luma;
        let fused_slice: &mut [u8] = &mut fused_img;
        luma_slice
            .par_iter_mut()
            .zip(fused_slice.par_iter_mut())
            .enumerate()
            .for_each(|(i, (l_out, f_out))| {
                let r = rgba_raw[i * 4] as f32;
                let g = rgba_raw[i * 4 + 1] as f32;
                let b = rgba_raw[i * 4 + 2] as f32;
                let mx = rgba_raw[i * 4].max(rgba_raw[i * 4 + 1]).max(rgba_raw[i * 4 + 2]);
                let mn = rgba_raw[i * 4].min(rgba_raw[i * 4 + 1]).min(rgba_raw[i * 4 + 2]);
                let l = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
                *l_out = l;
                *f_out = match fused_mode {
                    FusedChannel::A1 => (mx as i32 - (mn as i32) / 2) as u8,
                    FusedChannel::A2 => ((l as i32 + mx as i32 - mn as i32) / 2) as u8,
                };
            });
    }

    if SAVE_DEBUG_IMAGES.load(Ordering::Relaxed) {
        let _ = std::fs::create_dir_all("/tmp/qhints_debug");
        let _ = luma.save("/tmp/qhints_debug/01_luma.png");
    }

    // 2. Edge detection — a single Canny pass on the fused channel.
    let scale = rule.detection_scale;
    let w2 = (((w as f64) * scale) as u32).max(1);
    let h2 = (((h as f64) * scale) as u32).max(1);

    let fused_src = if scale != 1.0 {
        image::imageops::resize(&fused_img, w2, h2, image::imageops::FilterType::Nearest)
    } else {
        fused_img
    };

    let low = rule.canny_min_val as f32;
    let high = rule.canny_max_val as f32;

    let edges = canny_parallel(&fused_src, low, high);

    if SAVE_DEBUG_IMAGES.load(Ordering::Relaxed) {
        let _ = std::fs::create_dir_all("/tmp/qhints_debug");
        let _ = edges.save("/tmp/qhints_debug/02_edges.png");
    }

    // Text detection still uses luminance projection
    let luma_process = if scale != 1.0 {
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
    let dilated = dilate_parallel(&edges, radius);

    // 5. Connected components on dilated edges (parallel run-based CC), in the
    //    same order as the previous row-major BFS.
    let bboxes = connected_components_parallel(&dilated, img_w, img_h);
    let mut all_components: Vec<Child> = Vec::with_capacity(bboxes.len());
    for (min_x, min_y, max_x, max_y) in bboxes {
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

    // For each BFS component, track its best-overlapping word (max overlap).
    let n_words = word_rects.len();

    let overlap_results: Vec<(f64, Vec<usize>)> = all_components
        .par_iter()
        .map(|comp| {
            let cx = comp.relative_position.0;
            let cy = comp.relative_position.1;
            let cw = comp.width;
            let ch = comp.height;
            let area = cw * ch;
            if area <= 0.0 {
                return (0.0, Vec::new());
            }
            let mut best_overlap = 0.0f64;
            let mut overlapped = Vec::new();
            for (wi, &(wx, wy, ww, wh)) in word_rects.iter().enumerate() {
                let ix1 = cx.max(wx);
                let iy1 = cy.max(wy);
                let ix2 = (cx + cw).min(wx + ww);
                let iy2 = (cy + ch).min(wy + wh);
                if ix1 < ix2 && iy1 < iy2 {
                    let overlap = (ix2 - ix1) * (iy2 - iy1) / area;
                    overlapped.push(wi);
                    if overlap > best_overlap {
                        best_overlap = overlap;
                    }
                }
            }
            (best_overlap, overlapped)
        })
        .collect();

    let bfs_best_overlap: Vec<f64> = overlap_results.iter().map(|r| r.0).collect();
    let mut word_bfs_count = vec![0u32; n_words];
    for (_, overlapped) in &overlap_results {
        for &wi in overlapped {
            word_bfs_count[wi] += 1;
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

/// Canny edge detection with the heavy stages parallelized over the image via
/// rayon. Bit-identical to `imageproc::edges::canny` (same Gaussian kernel,
/// Sobel kernels, NMS, and hysteresis), just computed across multiple cores.
pub(crate) fn canny_parallel(img: &image::GrayImage, low: f32, high: f32) -> image::GrayImage {
    const SIGMA: f32 = 1.4;
    let kernel = gaussian_kernel(SIGMA);
    let blurred = blur1d(img, &kernel, true);
    let blurred = blur1d(&blurred, &kernel, false);

    let gx = sobel3x3(&blurred, &HORIZONTAL_SOBEL);
    let gy = sobel3x3(&blurred, &VERTICAL_SOBEL);

    let (w, h) = blurred.dimensions();
    let (w, h) = (w as usize, h as usize);
    let gx = gx.into_raw();
    let gy = gy.into_raw();

    // Gradient magnitude.
    let mut mag = vec![0.0f32; w * h];
    mag.par_iter_mut().enumerate().for_each(|(i, m)| {
        *m = (gx[i] as f32).hypot(gy[i] as f32);
    });

    // Non-maximum suppression (parallel per-pixel).
    let thinned = non_max_suppression(&mag, &gx, &gy, w, h);

    hysteresis(&thinned, low, high, w, h)
}

/// 1-D Gaussian kernel (radius `ceil(2*sigma)`), same as imageproc.
fn gaussian_kernel(sigma: f32) -> Vec<f32> {
    let radius = (2.0 * sigma).ceil() as usize;
    let mut k = vec![0.0f32; 2 * radius + 1];
    for i in 0..=radius {
        let v = (2.0 * std::f32::consts::PI).sqrt().recip() * sigma.recip()
            * (-(i as f32).powi(2) / (2.0 * sigma.powi(2))).exp();
        k[radius + i] = v;
        k[radius - i] = v;
    }
    k
}

/// Separable 1-D blur along one axis (continuity padding), parallel per row/col.
fn blur1d(img: &image::GrayImage, kernel: &[f32], horizontal: bool) -> image::GrayImage {
    let (w, h) = img.dimensions();
    let (w, h) = (w as usize, h as usize);
    let src = img.as_raw();
    let half = (kernel.len() as i32) / 2;
    let mut out = vec![0u8; w * h];
    out.par_iter_mut().enumerate().for_each(|(i, o)| {
        let x = i % w;
        let y = i / w;
        let mut acc = 0.0f32;
        for (j, &k) in kernel.iter().enumerate() {
            let off = j as i32 - half;
            let p = if horizontal {
                src[y * w + ((x as i32 + off).clamp(0, w as i32 - 1) as usize)]
            } else {
                src[((y as i32 + off).clamp(0, h as i32 - 1) as usize) * w + x]
            };
            acc += p as f32 * k;
        }
        *o = acc.clamp(0.0, 255.0) as u8;
    });
    image::GrayImage::from_raw(w as u32, h as u32, out).unwrap()
}

/// 3×3 convolution (continuity padding) producing i16, parallel per-pixel.
fn sobel3x3(img: &image::GrayImage, kernel: &[i32; 9]) -> image::ImageBuffer<image::Luma<i16>, Vec<i16>> {
    let (w, h) = img.dimensions();
    let (w, h) = (w as usize, h as usize);
    let src = img.as_raw();
    let mut out = vec![0i16; w * h];
    out.par_iter_mut().enumerate().for_each(|(i, o)| {
        let x = i % w;
        let y = i / w;
        let mut acc = 0i32;
        for r in 0..3usize {
            for c in 0..3usize {
                let xp = (x as i32 + c as i32 - 1).clamp(0, w as i32 - 1) as usize;
                let yp = (y as i32 + r as i32 - 1).clamp(0, h as i32 - 1) as usize;
                acc += src[yp * w + xp] as i32 * kernel[r * 3 + c];
            }
        }
        *o = acc as i16;
    });
    image::ImageBuffer::from_raw(w as u32, h as u32, out).unwrap()
}

/// Sobel kernels (same as imageproc).
const HORIZONTAL_SOBEL: [i32; 9] = [-1, 0, 1, -2, 0, 2, -1, 0, 1];
const VERTICAL_SOBEL: [i32; 9] = [-1, -2, -1, 0, 0, 0, 1, 2, 1];

/// Binary max-filter dilation (LInf square structuring element), parallel.
/// Bit-identical to `imageproc::morphology::dilate` with `Norm::LInf` for
/// binary (0/255) input.
pub(crate) fn dilate_parallel(img: &image::GrayImage, radius: u8) -> image::GrayImage {
    let (w, h) = img.dimensions();
    let (w, h) = (w as usize, h as usize);
    let src = img.as_raw();
    let r = radius as i32;
    let mut out = vec![0u8; w * h];
    out.par_iter_mut().enumerate().for_each(|(i, o)| {
        let x = i % w;
        let y = i / w;
        let mut mx = 0u8;
        for dy in -r..=r {
            let yy = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
            for dx in -r..=r {
                let xx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                mx = mx.max(src[yy * w + xx]);
            }
        }
        *o = mx;
    });
    image::GrayImage::from_raw(w as u32, h as u32, out).unwrap()
}

/// Parallel 4-connected-component labeling on a binary image, using a
/// run-length + union-find approach. Returns component bounding boxes
/// `(min_x, min_y, max_x, max_y)` in the same order as a row-major BFS scan
/// (sorted by first pixel), so output is bit-identical to the sequential
/// flood-fill it replaces.
pub(crate) fn connected_components_parallel(
    dilated: &image::GrayImage,
    img_w: u32,
    img_h: u32,
) -> Vec<(u32, u32, u32, u32)> {
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[rb] = ra;
        }
    }

    // 1. Extract horizontal runs per row (parallel).
    let runs_per_row: Vec<Vec<(u32, u32)>> = (0..img_h)
        .into_par_iter()
        .map(|y| {
            let mut runs = Vec::new();
            let mut x = 0u32;
            while x < img_w {
                if dilated.get_pixel(x, y)[0] > 0 {
                    let start = x;
                    while x < img_w && dilated.get_pixel(x, y)[0] > 0 {
                        x += 1;
                    }
                    runs.push((start, x - 1));
                } else {
                    x += 1;
                }
            }
            runs
        })
        .collect();

    // 2. Flatten runs; assign global ids; record (y, x_start, x_end).
    let mut runs: Vec<(u32, u32, u32)> = Vec::new();
    let mut row_start = vec![0usize; img_h as usize + 1];
    for (y, row_runs) in runs_per_row.into_iter().enumerate() {
        row_start[y + 1] = row_start[y] + row_runs.len();
        for (xs, xe) in row_runs {
            runs.push((y as u32, xs, xe));
        }
    }

    let n_runs = runs.len();
    let mut parent: Vec<usize> = (0..n_runs).collect();

    // 3. Union overlapping runs in adjacent rows (vertical connectivity).
    for y in 0..img_h.saturating_sub(1) {
        let (a0, a1) = (row_start[y as usize], row_start[y as usize + 1]);
        let (b0, b1) = (row_start[y as usize + 1], row_start[y as usize + 2]);
        let mut j = b0;
        for i in a0..a1 {
            let a_start = runs[i].1;
            let a_end = runs[i].2;
            while j < b1 && runs[j].2 < a_start {
                j += 1;
            }
            let mut k = j;
            while k < b1 && runs[k].1 <= a_end {
                union(&mut parent, i, k);
                k += 1;
            }
        }
    }

    // 4. Assign sequential component ids.
    let mut comp_id: Vec<usize> = vec![usize::MAX; n_runs];
    let mut map: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut next_id = 0usize;
    for (i, comp) in comp_id.iter_mut().enumerate() {
        let root = find(&mut parent, i);
        let id = *map.entry(root).or_insert_with(|| {
            let v = next_id;
            next_id += 1;
            v
        });
        *comp = id;
    }

    // 5. Per-component bounding box + first pixel (row-major order).
    let mut bbox: Vec<(u32, u32, u32, u32, u32, u32)> = vec![(u32::MAX, u32::MAX, 0, 0, 0, 0); next_id];
    let mut seen = vec![false; next_id];
    for i in 0..n_runs {
        let (y, xs, xe) = runs[i];
        let c = comp_id[i];
        if !seen[c] {
            seen[c] = true;
            bbox[c].4 = y;
            bbox[c].5 = xs;
        }
        bbox[c].0 = bbox[c].0.min(xs);
        bbox[c].1 = bbox[c].1.min(y);
        bbox[c].2 = bbox[c].2.max(xe);
        bbox[c].3 = bbox[c].3.max(y);
    }

    // 6. Emit in BFS (first-pixel) order.
    let mut order: Vec<usize> = (0..next_id).collect();
    order.sort_by_key(|&c| (bbox[c].4, bbox[c].5));
    order
        .into_iter()
        .map(|c| (bbox[c].0, bbox[c].1, bbox[c].2, bbox[c].3))
        .collect()
}

/// Thin edges by keeping local maxima along the gradient direction (parallel).
fn non_max_suppression(
    mag: &[f32],
    gx: &[i16],
    gy: &[i16],
    w: usize,
    h: usize,
) -> Vec<f32> {
    const RAD: f32 = 180.0 / std::f32::consts::PI;
    let mut out = vec![0.0f32; w * h];
    out.par_iter_mut().enumerate().for_each(|(i, o)| {
        let x = i % w;
        let y = i / w;
        if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            return;
        }
        let xg = gx[i] as f32;
        let yg = gy[i] as f32;
        let mut angle = yg.atan2(xg) * RAD;
        if angle < 0.0 {
            angle += 180.0;
        }
        let clamped = if !(22.5..157.5).contains(&angle) {
            0
        } else if (22.5..67.5).contains(&angle) {
            45
        } else if (67.5..112.5).contains(&angle) {
            90
        } else {
            135
        };
        let (i1, i2) = match clamped {
            0 => ((x - 1, y), (x + 1, y)),
            45 => ((x + 1, y + 1), (x - 1, y - 1)),
            90 => ((x, y - 1), (x, y + 1)),
            _ => ((x - 1, y + 1), (x + 1, y - 1)),
        };
        let c1 = mag[i1.1 * w + i1.0];
        let c2 = mag[i2.1 * w + i2.0];
        let p = mag[i];
        *o = if p < c1 || p < c2 { 0.0 } else { p };
    });
    out
}

/// Hysteresis thresholding (flood-fill, sequential — same as imageproc).
fn hysteresis(input: &[f32], low: f32, high: f32, w: usize, h: usize) -> image::GrayImage {
    let mut out = vec![0u8; w * h];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            if input[i] >= high && out[i] == 0 {
                out[i] = 255;
                stack.push((x, y));
                while let Some((nx, ny)) = stack.pop() {
                    let neighbors = [
                        (nx + 1, ny),
                        (nx + 1, ny + 1),
                        (nx, ny + 1),
                        (nx - 1, ny - 1),
                        (nx - 1, ny),
                        (nx - 1, ny + 1),
                    ];
                    for n in neighbors {
                        let ni = n.1 * w + n.0;
                        if input[ni] >= low && out[ni] == 0 {
                            out[ni] = 255;
                            stack.push(n);
                        }
                    }
                }
            }
        }
    }
    image::GrayImage::from_raw(w as u32, h as u32, out).unwrap()
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
    let edges_raw = edges.as_raw();
    let img_w_usize = img_w as usize;
    let mut row_sums = vec![0u32; img_h as usize];
    row_sums.par_iter_mut().enumerate().for_each(|(y, sum)| {
        let mut s = 0u32;
        let row = &edges_raw[y * img_w_usize..(y + 1) * img_w_usize];
        for &p in row {
            if p > 0 {
                s += 1;
            }
        }
        *sum = s;
    });

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
        let mut col_sums = vec![0u32; img_w_usize];
        col_sums.par_iter_mut().enumerate().for_each(|(x, sum)| {
            let mut s = 0u32;
            for y in ly0..ly1 {
                if edges_raw[(y as usize) * img_w_usize + x] > 0 {
                    s += 1;
                }
            }
            *sum = s;
        });

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
