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

    // Build luma image for BFS + edges (icons & non-text)
    let mut luma = image::GrayImage::new(w as u32, h as u32);
    let mut rgb = vec![0u8; (w as u32 * h as u32 * 3) as usize];
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

    let mut children: Vec<Child> = word_rects
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

    log::debug!("ocrs: {} text word rects", children.len());

    // ── BFS edge components for icons & non-text ───────────────────────
    let edges = imageproc::edges::canny(
        &luma,
        rule.canny_min_val as f32,
        rule.canny_max_val as f32,
    );
    let radius = (rule.kernel_size / 2) as u8;
    let dilated = imageproc::morphology::dilate(
        &edges,
        imageproc::distance_transform::Norm::LInf,
        radius,
    );

    let img_w = w as u32;
    let img_h = h as u32;
    let mut visited = vec![false; (img_w * img_h) as usize];

    for start_y in 0..img_h {
        for start_x in 0..img_w {
            let idx = (start_y * img_w + start_x) as usize;
            if visited[idx] || dilated.get_pixel(start_x, start_y)[0] == 0 {
                continue;
            }

            let mut min_x_bfs = start_x;
            let mut min_y_bfs = start_y;
            let mut max_x_bfs = start_x;
            let mut max_y_bfs = start_y;
            let mut queue = std::collections::VecDeque::new();
            queue.push_back((start_x, start_y));
            visited[idx] = true;

            while let Some((cx, cy)) = queue.pop_front() {
                if cx < min_x_bfs { min_x_bfs = cx; }
                if cy < min_y_bfs { min_y_bfs = cy; }
                if cx > max_x_bfs { max_x_bfs = cx; }
                if cy > max_y_bfs { max_y_bfs = cy; }

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

            let child_w = (max_x_bfs - min_x_bfs + 1) as f64;
            let child_h = (max_y_bfs - min_y_bfs + 1) as f64;

            // Keep all BFS components — the overlay's overlap culling handles
            // visual duplicates with OCR word boxes.
            children.push(Child {
                    absolute_position: (
                        (x + min_x_bfs as i32) as f64,
                        (y + min_y_bfs as i32) as f64,
                    ),
                    relative_position: (min_x_bfs as f64, min_y_bfs as f64),
                    width: child_w,
                    height: child_h,
                    kind: ChildKind::Element,
                });
            }
        }

    log::debug!("ocrs: {} total children (text + icons)", children.len());
    Ok(children)
}
