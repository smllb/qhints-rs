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

use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
use x11rb::rust_connection::RustConnection;

/// Capture the focused window region as an RGBA image via X11.
pub fn capture_window_image(
    window_info: &WindowInfo,
) -> Result<image::DynamicImage, Box<dyn std::error::Error>> {
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

    // X11 returns BGRA; reorder to RGBA.
    let mut rgba = image::RgbaImage::new(w as u32, h as u32);
    for (i, chunk) in data.chunks_exact(4).enumerate() {
        if i >= (w * h) as usize { break; }
        let cx = (i as u32) % (w as u32);
        let cy = (i as u32) / (w as u32);
        rgba.put_pixel(cx, cy, image::Rgba([chunk[2], chunk[1], chunk[0], 255]));
    }

    Ok(image::DynamicImage::ImageRgba8(rgba))
}

/// Capture the focused window via X11 and detect UI elements within it.
pub fn get_children(
    window_info: &WindowInfo,
    rule: &ApplicationRule,
) -> Result<Vec<Child>, Box<dyn std::error::Error>> {
    let (x, y, _, _) = window_info.extents;
    let img = capture_window_image(window_info)?;
    detect_children(&img, rule, x as f64, y as f64)
}

/// Intermediate results from `detect_children_debug`, exposing the pipeline
/// stages (luma, Canny edges, raw pieces, merged groups) so callers
/// (e.g. the screenshot benchmark) can render debug output.
pub struct DetectionDebug {
    pub children: Vec<Child>,
    pub luma: image::GrayImage,
    pub edges: image::GrayImage,
    /// Raw connected components — the hint targets.
    pub pieces: Vec<Child>,
    /// Estimated text height in pixels.
    pub text_h: f64,
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
    //    - fused (single contrast channel) for edge detection: max − min/2,
    //      where max/min are the brightest/darkest of R, G, B. This single
    //      channel keeps both dark-on-light edges (max domain) and bright
    //      saturated text edges (min domain, e.g. orange on white) that a
    //      plain max-of-RGB channel is blind to, without a second Canny pass.
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
                *l_out = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
                *f_out = (mx as i32 - (mn as i32) / 2) as u8;
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

    // 3. Dilate edges to bridge small gaps in strokes (kernel_size controls
    //    the radius). Kept small so glyphs/words stay mostly separate pieces.
    let img_w = w2;
    let img_h = h2;
    let inv_scale = 1.0 / scale;
    let radius = (rule.kernel_size / 2) as u8;
    let dilated = dilate_parallel(&edges, radius);

    // 4. Connected components (parallel run-based CC) → raw "pieces".
    let comps = connected_components_parallel(&dilated, img_w, img_h);
    let mut pieces: Vec<Child> = Vec::with_capacity(comps.len());
    for (min_x, min_y, max_x, max_y, _area) in comps {
        let rpx = (min_x as f64 * inv_scale).floor();
        let rpy = (min_y as f64 * inv_scale).floor();
        let cw = ((max_x - min_x + 1) as f64) * inv_scale;
        let ch = ((max_y - min_y + 1) as f64) * inv_scale;
        pieces.push(Child {
            absolute_position: (origin_x + rpx, origin_y + rpy),
            relative_position: (rpx, rpy),
            width: cw.ceil(),
            height: ch.ceil(),
            kind: ChildKind::Element,
        });
    }

    // Filter noise + giant containers.
    let max_container_w = w as f64 * 0.5;
    let max_container_h = h as f64 * 0.5;
    let pre_len = pieces.len();
    pieces.retain(|c| {
        c.width >= 1.0
            && c.height >= 1.0
            && c.width < max_container_w
            && c.height < max_container_h
    });
    if pieces.len() < pre_len {
        log::debug!(
            "Filtered {} tiny/large pieces",
            pre_len - pieces.len()
        );
    }

    // Pieces are the hint targets. All are Element.
    let children = pieces.clone();
    let text_h = estimate_text_height(&pieces, rule.text_height_min, rule.text_height_max);

    if let Ok(mut debug_bfs) = DEBUG_BFS_COMPONENTS.lock() {
        *debug_bfs = pieces.clone();
    }

    log::debug!(
        "imageproc: {} pieces, text_h={:.0}px",
        pieces.len(),
        text_h
    );

    // Debug images
    if SAVE_DEBUG_IMAGES.load(Ordering::Relaxed) {
        if let Ok(bfs) = DEBUG_BFS_COMPONENTS.lock() {
            if !bfs.is_empty() {
                let _ = draw_boxes(
                    &luma,
                    &pieces,
                    &children,
                    "/tmp/qhints_debug/04_bfs_debug.png",
                );
            }
        }
    }

    Ok(DetectionDebug {
        children,
        luma,
        edges,
        pieces,
        text_h,
    })
}

/// Estimate text height as the height bin with the most pieces within a ±3px
/// window (a sliding mode), over heights in `[min, max]`. Falls back to the
/// median piece height when few pieces qualify.
fn estimate_text_height(pieces: &[Child], min: f64, max: f64) -> f64 {
    let mut counts: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    for p in pieces {
        let h = p.height.round() as u32;
        if h >= min as u32 && h <= max as u32 {
            *counts.entry(h).or_insert(0) += 1;
        }
    }
    if counts.is_empty() {
        let mut hs: Vec<f64> = pieces.iter().map(|p| p.height).collect();
        hs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        return hs.get(hs.len() / 2).copied().unwrap_or(14.0);
    }
    let lo = min as u32;
    let hi = max as u32;
    let mut best_h = 14u32;
    let mut best = 0u32;
    for h in lo..=hi {
        let w: u32 = (h.saturating_sub(3)..=(h + 3).min(hi))
            .map(|k| counts.get(&k).copied().unwrap_or(0))
            .sum();
        if w > best {
            best = w;
            best_h = h;
        }
    }
    best_h as f64
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
/// `(min_x, min_y, max_x, max_y, area)` where `area` is the number of filled
/// pixels, in the same order as a row-major BFS scan (sorted by first pixel),
/// so output is bit-identical to the sequential flood-fill it replaces.
pub(crate) fn connected_components_parallel(
    dilated: &image::GrayImage,
    img_w: u32,
    img_h: u32,
) -> Vec<(u32, u32, u32, u32, u64)> {
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

    // 5. Per-component bounding box + first pixel (row-major order) + area.
    let mut bbox: Vec<(u32, u32, u32, u32, u32, u32, u64)> =
        vec![(u32::MAX, u32::MAX, 0, 0, 0, 0, 0); next_id];
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
        bbox[c].6 += (xe - xs + 1) as u64;
    }

    // 6. Emit in BFS (first-pixel) order.
    let mut order: Vec<usize> = (0..next_id).collect();
    order.sort_by_key(|&c| (bbox[c].4, bbox[c].5));
    order
        .into_iter()
        .map(|c| (bbox[c].0, bbox[c].1, bbox[c].2, bbox[c].3, bbox[c].6))
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
    pieces: &[Child],
    children: &[Child],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut img = image::RgbaImage::from_fn(luma.width(), luma.height(), |x, y| {
        let l = luma.get_pixel(x, y)[0];
        image::Rgba([l, l, l, 255])
    });
    // Raw pieces: red border
    for c in pieces {
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
    // Final children: blue border for Text, green for Element
    for c in children {
        let (x0, y0) = (c.relative_position.0 as u32, c.relative_position.1 as u32);
        let (x1, y1) = ((x0 + c.width as u32).saturating_sub(1), (y0 + c.height as u32).saturating_sub(1));
        let color = match c.kind {
            ChildKind::Text => [0u8, 120, 255],
            ChildKind::Element => [0u8, 200, 0],
        };
        for x in x0..=x1.min(img.width() - 1) {
            img.put_pixel(x, y0, image::Rgba([color[0], color[1], color[2], 255]));
            img.put_pixel(x, y1, image::Rgba([color[0], color[1], color[2], 255]));
        }
        for y in y0..=y1.min(img.height() - 1) {
            img.put_pixel(x0, y, image::Rgba([color[0], color[1], color[2], 255]));
            img.put_pixel(x1, y, image::Rgba([color[0], color[1], color[2], 255]));
        }
    }
    img.save(path)?;
    Ok(())
}
