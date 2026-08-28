//! Screenshot benchmark tests.
//!
//! Each PNG in `test-assets/screenshots/` is run through the imageproc
//! detection pipeline (detect → tiny filter → overlap culling), producing the
//! final hint count a user would actually see. The expected range is encoded
//! in the filename:
//!
//! - `30_min.png`           → at least 30 hints
//! - `30_min_60_max.png`    → between 30 and 60 hints
//! - `foo_10_min.png`       → at least 10 hints (default min is 30)
//!
//! Annotated output images and a report are written to `target/benchmarks/`.
//! If no screenshots are present, the test passes with a note.

use qhints_rs::backend::imageproc;
use qhints_rs::child::{Child, ChildKind};
use qhints_rs::config::{ApplicationRule, Config};
use qhints_rs::filter;
use qhints_rs::hints;

use std::path::{Path, PathBuf};

const SCREENSHOTS_DIR: &str = "test-assets/screenshots";
const OUTPUT_DIR: &str = "target/benchmarks";
const DEFAULT_MIN: u32 = 30;

/// Parse `Nmin` / `Mmax` bounds from a filename stem.
/// Returns `(min, max)` where `min` defaults to `DEFAULT_MIN` and `max`
/// is `None` when no `max` token is present.
fn parse_bounds(stem: &str) -> (u32, Option<u32>) {
    let min = bound_before(stem, "min").unwrap_or(DEFAULT_MIN);
    let max = bound_before(stem, "max");
    (min, max)
}

/// Strip any existing `Nmin` / `Mmax` tokens from a stem so a fresh baseline
/// can be appended without stacking duplicate bounds.
fn strip_bounds(stem: &str) -> String {
    let mut parts: Vec<&str> = stem.split(|c| c == '_' || c == '-').collect();
    let mut i = 0;
    while i < parts.len() {
        if parts[i].eq_ignore_ascii_case("min") || parts[i].eq_ignore_ascii_case("max") {
            parts.remove(i);
            if i > 0 && parts[i - 1].chars().all(|c| c.is_ascii_digit()) {
                parts.remove(i - 1);
                i -= 1;
            }
        } else {
            i += 1;
        }
    }
    parts.join("_").trim_matches('_').to_string()
}

/// Extract the number immediately preceding `key` (ignoring separators).
fn bound_before(stem: &str, key: &str) -> Option<u32> {
    let lower = stem.to_lowercase();
    let idx = lower.find(key)?;
    let before = &lower[..idx];
    let digits: String = before
        .chars()
        .rev()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    digits.parse().ok().filter(|n: &u32| *n > 0)
}

fn screenshots_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(SCREENSHOTS_DIR)
}

fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(OUTPUT_DIR)
}

#[test]
fn screenshot_benchmarks() {
    let update = std::env::var("UPDATE_BASELINES").map_or(false, |v| v == "1" || v == "true")
        || std::env::args().any(|a| a == "--update-baselines" || a == "--update");
    let dir = screenshots_dir();
    let mut screenshots: Vec<PathBuf> = if dir.exists() {
        std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("png"))
            .collect()
    } else {
        Vec::new()
    };
    screenshots.sort();

    if screenshots.is_empty() {
        eprintln!(
            "Screenshot benchmarks: no PNGs in {} — skipping (drop screenshots there to enable).",
            SCREENSHOTS_DIR
        );
        return;
    }

    let out = output_dir();
    std::fs::create_dir_all(&out).unwrap();

    let config = Config::default();
    let mut rule = ApplicationRule::default();
    rule.detection_scale = std::env::var("DETECTION_SCALE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let overlap_limit = filter::overlap_limit(config.hints.hint_overlap_threshold);

    let mut report = String::from("screenshot,min,max,raw_hints,final_hints,duration_ms,status\n");
    let mut failures: Vec<String> = Vec::new();

    for path in &screenshots {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let (min, max) = parse_bounds(&stem);

        let img = match image::open(path) {
            Ok(i) => i,
            Err(e) => {
                failures.push(format!("{}: failed to load image: {}", stem, e));
                continue;
            }
        };
        let (w, h) = (img.width(), img.height());

        let t0 = std::time::Instant::now();
        let debug = match imageproc::detect_children_debug(&img, &rule, 0.0, 0.0) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("{}: detection error: {}", stem, e));
                continue;
            }
        };
        let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let children = debug.children;

        let _ = debug.luma.save(out.join(format!("{}.01_luma.png", stem)));
        let _ = debug.edges.save(out.join(format!("{}.02_edges.png", stem)));
        let _ = imageproc::draw_boxes(
            &debug.luma,
            &debug.words,
            &debug.all_bfs,
            &children,
            out.join(format!("{}.04_bfs_debug.png", stem))
                .to_str()
                .unwrap(),
        );

        let children = filter::filter_tiny(children, w as f64, h as f64);
        let kept = filter::cull_overlaps(&children, overlap_limit);
        let survivors: Vec<&Child> = children
            .iter()
            .zip(&kept)
            .filter(|(_, &k)| k)
            .map(|(c, _)| c)
            .collect();
        let hint_count = survivors.len() as u32;

        if update {
            let base = strip_bounds(&stem);
            let new_stem = if base.is_empty() {
                format!("{}_min_{}_max", hint_count, hint_count)
            } else {
                format!("{}_{}_min_{}_max", base, hint_count, hint_count)
            };
            let new_path = path.with_file_name(format!("{}.png", new_stem));
            std::fs::rename(path, &new_path).unwrap();
            println!("{}: {} final hints -> {}", stem, hint_count, new_stem);
            report.push_str(&format!(
                "{},{},{},{},{},{:.2},baseline\n",
                stem,
                min,
                max.map_or("-".to_string(), |m| m.to_string()),
                children.len(),
                hint_count,
                duration_ms
            ));
            continue;
        }

        let survivor_children: Vec<Child> = survivors.iter().map(|c| (*c).clone()).collect();
        let hint_map = hints::get_hints(
            &survivor_children,
            &config.complementary_keys_alphabet,
            &config.first_key_zones,
            &config.center_zone_padding,
            Some((w as f64, h as f64)),
        );

        let ok = hint_count >= min && max.map_or(true, |m| hint_count <= m);
        let status = if ok { "ok" } else { "FAIL" };

        let annotate_path = out.join(format!("{}.annotated.png", stem));
        let _ = draw_annotated(
            &img,
            &children,
            &kept,
            &annotate_path,
        );

        println!(
            "{}: raw={} final_hints={} [min={}, max={}] {:.2}ms -> {}",
            stem,
            children.len(),
            hint_count,
            min,
            max.map_or("-".to_string(), |m| m.to_string()),
            duration_ms,
            status
        );
        report.push_str(&format!(
            "{},{},{},{},{},{:.2},{}\n",
            stem, min, max.map_or("-".to_string(), |m| m.to_string()),
            children.len(), hint_count, duration_ms, status
        ));

        if !ok {
            let mut labels: Vec<&str> = hint_map.keys().map(|s| s.as_str()).collect();
            labels.sort();
            failures.push(format!(
                "{}: final hints {} out of range [min={}, max={}] (labels: {})",
                stem,
                hint_count,
                min,
                max.map_or("∞".to_string(), |m| m.to_string()),
                labels.join(",")
            ));
        }
    }

    std::fs::write(out.join("report.csv"), &report).unwrap();
    println!("Benchmark report written to {}/report.csv", OUTPUT_DIR);

    if update {
        println!("Baselines updated for {} screenshot(s).", screenshots.len());
        return;
    }

    assert!(
        failures.is_empty(),
        "{} screenshot benchmark(s) failed:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Draw final hints (green=Element, blue=Text) and culled children (red).
fn draw_annotated(
    img: &image::DynamicImage,
    children: &[Child],
    kept: &[bool],
    path: &Path,
) -> Result<(), image::ImageError> {
    let mut out = img.to_rgba8();
    let (w, h) = out.dimensions();

    for (i, c) in children.iter().enumerate() {
        let (color, thick) = if kept[i] {
            match c.kind {
                ChildKind::Text => ([0u8, 120, 255], 2u32),
                ChildKind::Element => ([0, 200, 0], 2),
            }
        } else {
            ([255, 0, 0], 1)
        };

        let x0 = (c.relative_position.0 as u32).min(w.saturating_sub(1));
        let y0 = (c.relative_position.1 as u32).min(h.saturating_sub(1));
        let x1 = (c.relative_position.0 as u32 + c.width as u32).min(w.saturating_sub(1));
        let y1 = (c.relative_position.1 as u32 + c.height as u32).min(h.saturating_sub(1));

        for t in 0..thick {
            let (xt0, yt0) = (x0 + t, y0 + t);
            let (xt1, yt1) = (x1.saturating_sub(t), y1.saturating_sub(t));
            for x in xt0..=xt1 {
                out.put_pixel(x, yt0, image::Rgba([color[0], color[1], color[2], 255]));
                out.put_pixel(x, yt1, image::Rgba([color[0], color[1], color[2], 255]));
            }
            for y in yt0..=yt1 {
                out.put_pixel(xt0, y, image::Rgba([color[0], color[1], color[2], 255]));
                out.put_pixel(xt1, y, image::Rgba([color[0], color[1], color[2], 255]));
            }
        }
    }

    out.save(path)
}
