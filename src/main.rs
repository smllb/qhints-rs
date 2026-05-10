mod backend;
mod child;
mod config;
mod hints;
mod mouse;
mod overlay;
mod window_system;

use crate::child::ChildKind;
use crate::window_system::WindowSystem;
use clap::Parser;
use std::fs::OpenOptions;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "qhints-rs", about = "Keyboard-driven UI navigation for Linux")]
struct Cli {
    /// Mode: hint or scroll
    #[arg(short, long, default_value = "hint")]
    mode: String,

    /// Verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn try_acquire_lock() -> Option<std::fs::File> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open("/tmp/qhints.lock")
        .ok()?;

    use std::os::unix::io::AsRawFd;
    let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };

    if ret == 0 { Some(file) } else { None }
}

/// Run a blocking operation in a separate thread with a hard timeout.
/// Returns None if the operation times out or the thread panics.
///
/// **Note:** The spawned thread is NOT cancelled on timeout — it continues
/// running in the background until `f()` returns.  This is a fundamental
/// limitation of `std::thread` (Rust provides no forcible thread-kill API).
/// In practice this is acceptable because:
///   - `f()` always has an internal bound (e.g. 5s imageproc timeout).
///   - The lock file prevents concurrent invocations, so at most 2–3
///     orphaned threads can exist at once, all of which terminate quickly.
fn with_thread_timeout<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
    timeout: std::time::Duration,
    label: &'static str,
) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(timeout) {
        Ok(val) => Some(val),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            log::error!("{} timed out after {:?}", label, timeout);
            None
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            log::error!("{} thread panicked or disconnected", label);
            None
        }
    }
}

fn main() {
    let total_start = Instant::now();

    let cli = Cli::parse();

    // Initialize logging
    let log_level = match cli.verbose {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };
    env_logger::Builder::new().filter_level(log_level).init();

    // Load config
    let t = Instant::now();
    let config = config::load_config();
    log::debug!("Config loaded in {:?}", t.elapsed());

    match cli.mode.as_str() {
        "hint" => hint_mode(&config, total_start),
        "scroll" => {
            log::warn!("Scroll mode not yet implemented in Rust binary");
        }
        _ => {
            log::error!("Unknown mode: {}", cli.mode);
        }
    }
}

fn hint_mode(config: &config::Config, total_start: Instant) {
    // Prevent re-entry if overlay is already active
    let _lock = match try_acquire_lock() {
        Some(f) => f,
        None => {
            log::warn!("qhints already running, ignoring trigger");
            return;
        }
    };

    // Initialize X11 window system (with 2s hard timeout)
    let t = Instant::now();
    let ws = match with_thread_timeout(
        || match window_system::x11::X11::new() {
            Ok(ws) => Ok(ws),
            Err(e) => Err(format!("{}", e)),
        },
        std::time::Duration::from_secs(2),
        "X11 init",
    ) {
        Some(Ok(ws)) => ws,
        Some(Err(e)) => {
            log::error!("Failed to initialize X11: {}", e);
            return;
        }
        None => {
            log::error!("X11 init timed out");
            return;
        }
    };
    log::debug!("X11 init in {:?}", t.elapsed());

    let win_info = ws.focused_window().clone();
    log::debug!(
        "Active window: '{}' (PID {}) at {:?}",
        win_info.app_name,
        win_info.pid,
        win_info.extents
    );

    // Get application rules (use app-specific or default)
    let rule = config
        .application_rules
        .get(&win_info.app_name)
        .cloned()
        .unwrap_or_else(|| {
            config
                .application_rules
                .get("default")
                .cloned()
                .unwrap_or_default()
        });

    // ── Hunt loop: re‑scan + re‑label + show until Ctrl signals exit ──
    loop {
    // AT-SPI tree walk (async, with hard thread-level deadline)
    let t = Instant::now();
    let mut children = {
        let (tx, rx) = std::sync::mpsc::channel();
        let win_info_clone = win_info.clone();
        let rule_clone = rule.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            let result = rt.block_on(async {
                match tokio::time::timeout(std::time::Duration::from_millis(150), async {
                    let backend = backend::atspi::AtspiBackend::new(win_info_clone, rule_clone).await?;
                    backend.get_children().await
                }).await {
                    Ok(Ok(children)) => Some(children),
                    Ok(Err(e)) => {
                        log::debug!("AT-SPI error: {}", e);
                        None
                    }
                    Err(_) => {
                        log::debug!("AT-SPI tokio timeout after 150ms");
                        None
                    }
                }
            });
            let _ = tx.send(result);
        });

        match rx.recv_timeout(std::time::Duration::from_millis(250)) {
            Ok(Some(c)) => c,
            _ => {
                log::debug!("AT-SPI hard deadline exceeded (250ms)");
                Vec::new()
            }
        }
    };
    log::debug!("AT-SPI tree walk: {:?} ({} children)", t.elapsed(), children.len());

    // Run all configured fallback backends (in order, skip atspi which ran above).
    // Results are merged: OCR text takes priority over BFS in the overlap culling below.
    for backend_name in &config.backends {
        if backend_name == "atspi" {
            continue;
        }
        let cv_start = Instant::now();
        let (w, h) = (win_info.extents.2, win_info.extents.3);
        if w as u64 * h as u64 > 1920 * 1080 {
            log::warn!("Large image for {}: {}x{} — may block briefly", backend_name, w, h);
        }

        let new_children = match backend_name.as_str() {
            "imageproc" => {
                let win_info_clone = win_info.clone();
                let rule_clone = rule.clone();
                with_thread_timeout(
                    move || match backend::imageproc::get_children(&win_info_clone, &rule_clone) {
                        Ok(c) => Ok(c),
                        Err(e) => Err(format!("{}", e)),
                    },
                    std::time::Duration::from_secs(5),
                    "imageproc",
                )
                .and_then(|r| match r {
                    Ok(c) => Some(c),
                    Err(e) => {
                        log::error!("Imageproc fallback failed: {}", e);
                        None
                    }
                })
                .unwrap_or_else(|| {
                    log::error!("Imageproc fallback timed out or failed");
                    Vec::new()
                })
            }
            #[cfg(feature = "ocr")]
            "ocrs" => {
                let win_info_clone = win_info.clone();
                let rule_clone = rule.clone();
                with_thread_timeout(
                    move || match backend::ocrs::get_children(&win_info_clone, &rule_clone) {
                        Ok(c) => Ok(c),
                        Err(e) => Err(format!("{}", e)),
                    },
                    std::time::Duration::from_secs(15),
                    "ocrs",
                )
                .and_then(|r| match r {
                    Ok(c) => Some(c),
                    Err(e) => {
                        log::error!("OCR fallback failed: {}", e);
                        None
                    }
                })
                .unwrap_or_else(|| {
                    log::error!("OCR fallback timed out or failed");
                    Vec::new()
                })
            }
            _ => {
                log::warn!("Unknown backend: {}", backend_name);
                Vec::new()
            }
        };
        log::debug!("{} fallback: {:?} ({} children)", backend_name, cv_start.elapsed(), new_children.len());
        children.extend(new_children);
    }

    // Pre-filter noise: remove children smaller than 0.5% of screen dim
    // and merge children fully contained within adjacent larger ones
    let (_, _, w, h) = win_info.extents;
    let min_child_w = (w as f64 * 0.0025).max(3.0);
    let min_child_h = (h as f64 * 0.0025).max(3.0);
    let orig_len = children.len();
    children.retain(|c| c.width >= min_child_w && c.height >= min_child_h);
    if children.len() < orig_len {
        log::debug!("Filtered {} tiny children (now {})", orig_len - children.len(), children.len());
    }

    if children.is_empty() {
        log::debug!("No accessible children found");
        return;
    }

    // Compute hints
    let t = Instant::now();
    let mut hint_map = hints::get_hints(&children, &config.complementary_keys_alphabet, &config.first_key_zones, &config.center_zone_padding, Some((w as f64, h as f64)));
    log::debug!("Hint computation: {:?} ({} hints)", t.elapsed(), hint_map.len());

    // Re-label just the "real" visible survivors after overlap culling.
    // The raw 924 children produce 3-char hints, but only ~100 are actually
    // visible after overlap filtering. Re-label those with fresh short hints.
    let overlap_limit = if config.hints.hint_overlap_threshold == 0.0 {
        f64::MAX
    } else {
        (100.0 - config.hints.hint_overlap_threshold) / 100.0
    };

    // Build child rects indexed by original position
    let child_rects: Vec<(usize, f64, f64, f64, f64)> = children
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.relative_position.0, c.relative_position.1, c.width, c.height))
        .collect();

    // Pairwise overlap culling: keep the larger child when two overlap.
    // Text children are only preferred when the overlap is extreme (>80%).
    let mut kept = vec![true; children.len()];
    for i in 0..child_rects.len() {
        if !kept[i] { continue; }
        let (_, x1, y1, w1, h1) = child_rects[i];
        let area1 = w1 * h1;
        for j in (i + 1)..child_rects.len() {
            if !kept[j] { continue; }
            let (_, x2, y2, w2, h2) = child_rects[j];
            let ix1 = x1.max(x2);
            let iy1 = y1.max(y2);
            let ix2 = (x1 + w1).min(x2 + w2);
            let iy2 = (y1 + h1).min(y2 + h2);
            if ix1 < ix2 && iy1 < iy2 {
                let inter = (ix2 - ix1) * (iy2 - iy1);
                let area2 = w2 * h2;
                let min_area = area1.min(area2);
                if min_area > 0.0 && inter / min_area > overlap_limit {
                    // Only prefer Text over Element when overlap is extreme
                    // (>80% of the smaller element).  Otherwise keep both so
                    // nearby icons aren't silently erased by text word boxes.
                    let kind_i = children[i].kind;
                    let kind_j = children[j].kind;
                    let high_overlap = inter / min_area > 0.8;
                    if high_overlap {
                        if kind_i == ChildKind::Text && kind_j != ChildKind::Text {
                            kept[j] = false;
                            continue;
                        } else if kind_j == ChildKind::Text && kind_i != ChildKind::Text {
                            kept[i] = false;
                            break;
                        }
                    }
                    // Cull the SMALLER one
                    if area1 <= area2 {
                        kept[j] = false;
                    } else {
                        kept[i] = false;
                        break;
                    }
                }
            }
        }
    }

    let survivor_count = kept.iter().filter(|&&k| k).count();
    if survivor_count < children.len() {
        let survivor_indices: Vec<usize> = kept.iter()
            .enumerate()
            .filter(|(_, &k)| k)
            .map(|(i, _)| i)
            .collect();
        let survivor_children: Vec<child::Child> = survivor_indices
            .iter()
            .map(|&i| children[i].clone())
            .collect();
        let new_hints = hints::get_hints(
            &survivor_children,
            &config.complementary_keys_alphabet,
            &config.first_key_zones,
            &config.center_zone_padding,
            Some((w as f64, h as f64)),
        );
        hint_map = new_hints
            .into_iter()
            .map(|(label, idx)| (label, survivor_indices[idx]))
            .collect();
        log::debug!("Re-labeled {} survivors from {} raw (now {} hints)", survivor_count, children.len(), hint_map.len());
    }

    // Show overlay
    let (x, y, width, height) = win_info.extents;
    if let Some(action) = overlay::show_overlay(config, &hint_map, &children, x, y, width, height, None) {
        log::debug!("Action: {:?}", action);

        // Full-screen re-scan requested (drag mode, need to pick destination on whole screen)
        log::debug!("Action drag_fullscreen={}", action.drag_fullscreen);
        if action.drag_fullscreen {
            let (s_x, s_y) = (action.x, action.y);
            log::debug!("Full-screen re-scan requested for drag, source at ({}, {})", s_x, s_y);
            // Scan full screen
            let screen_win = match window_system::x11::screen_size() {
                Ok((sw, sh)) => {
                    log::debug!("Screen size: {}x{}", sw, sh);
                    window_system::WindowInfo {
                        extents: (0, 0, sw, sh),
                        pid: 0,
                        app_name: "__screen__".into(),
                    }
                },
                Err(e) => {
                    log::error!("Failed to get screen size: {}", e);
                    return;
                }
            };
            let screen_rule = config.application_rules.get("default").cloned().unwrap_or_default();
            let mut screen_children: Vec<child::Child> = Vec::new();
            for backend_name in &config.backends {
                if backend_name == "atspi" { continue; }
                match backend_name.as_str() {
                    "imageproc" => {
                        let win_clone = screen_win.clone();
                        let rule_clone = screen_rule.clone();
                        if let Some(Some(c)) = with_thread_timeout(
                            move || match backend::imageproc::get_children(&win_clone, &rule_clone) {
                                Ok(c) => Some(c),
                                Err(e) => { log::error!("imageproc error: {}", e); None }
                            },
                            std::time::Duration::from_secs(5), "imageproc",
                        ) {
                            screen_children.extend(c);
                        }
                    }
                    #[cfg(feature = "ocr")]
                    "ocrs" => {
                        let win_clone = screen_win.clone();
                        let rule_clone = screen_rule.clone();
                        if let Some(Some(c)) = with_thread_timeout(
                            move || match backend::ocrs::get_children(&win_clone, &rule_clone) {
                                Ok(c) => Some(c),
                                Err(e) => { log::error!("ocrs error: {}", e); None }
                            },
                            std::time::Duration::from_secs(15), "ocrs",
                        ) {
                            screen_children.extend(c);
                        }
                    }
                    _ => {}
                }
            }
            if screen_children.is_empty() {
                log::warn!("No children found in full-screen scan");
                return;
            }
            let min_w = (screen_win.extents.2 as f64 * 0.005).max(4.0);
            let min_h = (screen_win.extents.3 as f64 * 0.005).max(4.0);
            screen_children.retain(|c| c.width >= min_w && c.height >= min_h);
            if screen_children.is_empty() {
                log::warn!("No children survive filter in full-screen scan");
                return;
            }
            let screen_hints = hints::get_hints(&screen_children, &config.complementary_keys_alphabet, &config.first_key_zones, &config.center_zone_padding, Some((screen_win.extents.2 as f64, screen_win.extents.3 as f64)));
            if let Some(action2) = overlay::show_overlay(config, &screen_hints, &screen_children, 0, 0, screen_win.extents.2, screen_win.extents.3, Some((s_x, s_y))) {
                log::debug!("Full-screen drag action: {:?}", action2);
                match action2.action.as_str() {
                    "drag" => {
                        let delay = (config.hints.drag_delay_ms as f64) / 1000.0;
                        let dx = action2.end_x - action2.x;
                        let dy = action2.end_y - action2.y;
                        let dist = ((dx * dx + dy * dy) as f64).sqrt();
                let steps = (dist / 8.0).ceil() as i32;
                        let mut cmd = format!("xdotool mousemove {} {}; sleep {}; xdotool mousedown {}; sleep {}",
                            action2.x, action2.y, delay, action2.button, delay);
                        for s in 1..=steps {
                            let t = s as f64 / steps as f64;
                            let mx = action2.x + (dx as f64 * t) as i32;
                            let my = action2.y + (dy as f64 * t) as i32;
                            cmd.push_str(&format!("; xdotool mousemove {} {}", mx, my));
                        }
                        cmd.push_str(&format!("; sleep {}; xdotool mouseup {}", delay, action2.button));
                        log::debug!("xdotool cmd: {} steps", steps);
                        std::process::Command::new("sh")
                            .arg("-c").arg(&cmd)
                            .status()
                            .expect("Failed to spawn xdotool");
                    }
                    _ => log::debug!("Unhandled action: {}", action2.action),
                }
            }
            return;
        }

        log::debug!("Executing action: {:?}", action);
        match action.action.as_str() {
            "click" => {
                let mut cmd = format!("xdotool mousemove {} {} ", action.x, action.y);
                for _ in 0..action.repeat {
                    cmd.push_str(&format!("click {} ", action.button));
                }
                std::process::Command::new("sh")
                    .arg("-c").arg(&cmd)
                    .status()
                    .expect("Failed to spawn xdotool");
            }
            "hover" => {
                let cmd = format!("xdotool mousemove {} {}", action.x, action.y);
                std::process::Command::new("sh")
                    .arg("-c").arg(&cmd)
                    .status()
                    .expect("Failed to spawn xdotool");
            }
            "drag" | "select" => {
                let delay = (config.hints.drag_delay_ms as f64) / 1000.0;
                let dx = action.end_x - action.x;
                let dy = action.end_y - action.y;
                let dist = ((dx * dx + dy * dy) as f64).sqrt();
                let steps = (dist / 8.0).ceil() as i32;
                let mut cmd = format!("xdotool mousemove {} {}; sleep {}; xdotool mousedown {}; sleep {}",
                    action.x, action.y, delay, action.button, delay);
                for s in 1..=steps {
                    let t = s as f64 / steps as f64;
                    let mx = action.x + (dx as f64 * t) as i32;
                    let my = action.y + (dy as f64 * t) as i32;
                    cmd.push_str(&format!("; xdotool mousemove {} {}", mx, my));
                }
                cmd.push_str(&format!("; sleep {}; xdotool mouseup {}", delay, action.button));
                log::debug!("xdotool cmd: {} steps", steps);
                std::process::Command::new("sh")
                    .arg("-c").arg(&cmd)
                    .status()
                    .expect("Failed to spawn xdotool");
            }
            _ => {
                log::debug!("Unhandled action: {}", action.action);
            }
        }

        if !action.hunt_continue {
            break;
        }
        // Let the UI settle before re-scanning
        std::thread::sleep(std::time::Duration::from_millis(
            config.dev.hunt_timeout_ms as u64,
        ));
        continue;
    }
    break;
}
}