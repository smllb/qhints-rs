use crate::child::Child;
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

    // 4. Dilate
    let radius = (rule.kernel_size / 2) as u8;
    let dilated = imageproc::morphology::dilate(
        &edges,
        imageproc::distance_transform::Norm::LInf,
        radius,
    );

    // Debug dump
    let _ = std::fs::create_dir_all("/tmp/qhints_debug");
    let _ = luma.save("/tmp/qhints_debug/01_luma.png");
    let _ = edges.save("/tmp/qhints_debug/02_edges.png");
    let _ = dilated.save("/tmp/qhints_debug/03_dilated.png");

    // 5. BFS connected components on dilated image — fully deterministic
    let img_w = w as u32;
    let img_h = h as u32;
    let mut visited = vec![false; (img_w * img_h) as usize];
    let mut children = Vec::new();

    let min_dim = 8i32;
    let max_w = (w as f32 * 0.5) as i32;
    let max_h = (h as f32 * 0.5) as i32;

    // Scan top-to-bottom, left-to-right — deterministic ordering
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

                // 4-connected neighbors
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

            let child_w = (max_x - min_x + 1) as i32;
            let child_h = (max_y - min_y + 1) as i32;

            // Size filter
            // if child_w < min_dim || child_h < min_dim { continue; }
            // if child_w > max_w || child_h > max_h { continue; }

            children.push(Child {
                absolute_position: (
                    (x + min_x as i32) as f64,
                    (y + min_y as i32) as f64,
                ),
                relative_position: (min_x as f64, min_y as f64),
                width: child_w as f64,
                height: child_h as f64,
            });
        }
    }

    log::debug!("imageproc: {} children after BFS", children.len());

    Ok(children)
}