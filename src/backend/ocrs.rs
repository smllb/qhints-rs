use crate::child::Child;
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
    _rule: &ApplicationRule,
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

    let mut rgb = vec![0u8; (w as u32 * h as u32 * 3) as usize];
    for (i, chunk) in data.chunks_exact(4).enumerate() {
        if i >= (w as u32 * h as u32) as usize { break; }
        rgb[i * 3] = chunk[2];
        rgb[i * 3 + 1] = chunk[1];
        rgb[i * 3 + 2] = chunk[0];
    }

    let img_source = ocrs::ImageSource::from_bytes(&rgb, (w as u32, h as u32))?;
    let ocr_input = engine.prepare_input(img_source)?;

    let word_rects = engine.detect_words(&ocr_input)?;
    let line_rects = engine.find_text_lines(&ocr_input, &word_rects);

    let children: Vec<Child> = line_rects
        .iter()
        .flat_map(|line| line.iter())
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
            }
        })
        .collect();

    log::debug!("ocrs: {} word rects", children.len());
    Ok(children)
}
