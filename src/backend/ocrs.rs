use crate::child::{Child, ChildKind};
use crate::config::ApplicationRule;
use crate::window_system::WindowInfo;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
use x11rb::rust_connection::RustConnection;

const DETECTION_MODEL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
const RECOGNITION_MODEL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

fn cache_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    let dir = std::path::PathBuf::from(home).join(".cache/qhints/ocrs");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn download_model(url: &str, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        return Ok(());
    }
    log::info!("Downloading OCR model: {}", url);
    let resp = ureq::get(url).call()?;
    let mut body = resp.into_body();
    let mut file = std::fs::File::create(path)?;
    std::io::copy(&mut body.as_reader(), &mut file)?;
    Ok(())
}

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

    let mut rgb = vec![0u8; (w as u32 * h as u32 * 3) as usize];
    let mut luma = image::GrayImage::new(w as u32, h as u32);
    for (i, chunk) in data.chunks_exact(4).enumerate() {
        if i >= (w as u32 * h as u32) as usize { break; }
        let b = chunk[0] as f32;
        let g = chunk[1] as f32;
        let r = chunk[2] as f32;
        let l = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
        let cx = (i as u32) % (w as u32);
        let cy = (i as u32) / (w as u32);
        luma.put_pixel(cx, cy, image::Luma([l]));
        rgb[i * 3] = chunk[2];
        rgb[i * 3 + 1] = chunk[1];
        rgb[i * 3 + 2] = chunk[0];
    }

    // ── OCR text detection ─────────────────────────────────────────────
    let cache = cache_dir()?;
    let detect_path = cache.join("text-detection.rten");
    let rec_path = cache.join("text-recognition.rten");
    download_model(DETECTION_MODEL, &detect_path)?;
    download_model(RECOGNITION_MODEL, &rec_path)?;

    let detection_model = rten::Model::load_file(&detect_path)?;
    let recognition_model = rten::Model::load_file(&rec_path)?;

    let engine = ocrs::OcrEngine::new(ocrs::OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })?;

    let img_source = ocrs::ImageSource::from_bytes(&rgb, (w as u32, h as u32))?;
    let ocr_input = engine.prepare_input(img_source)?;
    let word_rects = engine.detect_words(&ocr_input)?;

    let word_children: Vec<Child> = word_rects
        .iter()
        .map(|word_bbox| {
            let corners = word_bbox.corners();
            let min_x = corners.iter().map(|p| p.x).fold(f32::MAX, f32::min) as f64;
            let max_x = corners.iter().map(|p| p.x).fold(f32::MIN, f32::max) as f64;
            let min_y = corners.iter().map(|p| p.y).fold(f32::MAX, f32::min) as f64;
            let max_y = corners.iter().map(|p| p.y).fold(f32::MIN, f32::max) as f64;
            Child {
                absolute_position: (x as f64 + min_x, y as f64 + min_y),
                relative_position: (min_x, min_y),
                width: (max_x - min_x).max(1.0),
                height: (max_y - min_y).max(1.0),
                kind: ChildKind::Text,
            }
        })
        .collect();

    log::debug!("ocrs: {} text word rects", word_children.len());

    // ── BFS edge components fill gaps where OCR found nothing ──────────
    let edges = crate::backend::imageproc::canny_parallel(
        &luma,
        rule.canny_min_val as f32,
        rule.canny_max_val as f32,
    );
    let radius = (rule.kernel_size / 2) as u8;
    let dilated = crate::backend::imageproc::dilate_parallel(&edges, radius);

    let img_w = w as u32;
    let img_h = h as u32;

    let mut bfs_children: Vec<Child> = crate::backend::imageproc::connected_components_parallel(
        &dilated,
        img_w,
        img_h,
    )
    .into_iter()
    .map(|(min_x_bfs, min_y_bfs, max_x_bfs, max_y_bfs)| Child {
        absolute_position: (
            (x + min_x_bfs as i32) as f64,
            (y + min_y_bfs as i32) as f64,
        ),
        relative_position: (min_x_bfs as f64, min_y_bfs as f64),
        width: (max_x_bfs - min_x_bfs + 1) as f64,
        height: (max_y_bfs - min_y_bfs + 1) as f64,
        kind: ChildKind::Element,
    })
    .collect();

    // Remove BFS components that substantially overlap OCR word boxes
    let word_rects: Vec<(f64, f64, f64, f64)> = word_children.iter().map(|c| {
        (c.relative_position.0, c.relative_position.1, c.width, c.height)
    }).collect();
    bfs_children.retain(|child| {
        let cx = child.relative_position.0;
        let cy = child.relative_position.1;
        let cw = child.width;
        let ch = child.height;
        let area = cw * ch;
        if area <= 0.0 { return false; }
        let max_overlap = word_rects.iter().map(|&(wx, wy, ww, wh)| {
            let ix1 = cx.max(wx);
            let iy1 = cy.max(wy);
            let ix2 = (cx + cw).min(wx + ww);
            let iy2 = (cy + ch).min(wy + wh);
            if ix1 < ix2 && iy1 < iy2 {
                (ix2 - ix1) * (iy2 - iy1) / area
            } else {
                0.0
            }
        }).fold(0.0f64, f64::max);
        max_overlap < 0.3
    });

    let bfs_count = bfs_children.len();
    let word_count = word_children.len();
    let mut children = bfs_children;
    children.extend(word_children);
    log::debug!("ocrs: {} total children ({} bfs + {} text)", children.len(), bfs_count, word_count);
    Ok(children)
}
