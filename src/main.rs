use qhints_rs::backend;
use qhints_rs::child;
use qhints_rs::child::ChildKind;
use qhints_rs::config;
use qhints_rs::filter;
use qhints_rs::hints;
use qhints_rs::overlay;
use qhints_rs::window_system;
use qhints_rs::window_system::WindowSystem;
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
        "hint" => hint_mode(&config),
        "scroll" => {
            log::warn!("Scroll mode not yet implemented in Rust binary");
        }
        _ => {
            log::error!("Unknown mode: {}", cli.mode);
        }
    }
}

fn hint_mode(config: &config::Config) {
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
    // Fallback results are kept separate: text children serve as reference to
    // classify BFS components, then discarded — only BFS shapes survive.
    let mut fallback_children: Vec<child::Child> = Vec::new();
    for backend_name in &config.backends {
        if backend_name == "atspi" {
            continue;
        }
        let cv_start = Instant::now();
        let (w, h) = (win_info.extents.2, win_info.extents.3);
        if w as u64 * h as u64 > 1920 * 1080 {
            log::warn!("Large image for {}: {}x{} — may block briefly", backend_name, w, h);
        }

        if backend_name == "imageproc" {
            backend::imageproc::SAVE_DEBUG_IMAGES.store(config.dev.save_debug_images, std::sync::atomic::Ordering::Relaxed);
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
        fallback_children.extend(new_children);
    }

    // Use fallback text references (imageproc detect_text_words, OCR) to
    // classify BFS components (Element → Text). Then discard only the
    // original backend Text references — BFS components that were converted
    // to Text survive with their new kind.
    // Track which indices in fallback_children are the original backend Text
    let backend_text_indices: Vec<usize> = fallback_children.iter()
        .enumerate()
        .filter(|(_, c)| c.kind == ChildKind::Text)
        .map(|(i, _)| i)
        .collect();
    // Convert BFS (Element) → Text where they overlap reference text by >95 %
    if !backend_text_indices.is_empty() {
        let ref_text: Vec<(f64, f64, f64, f64)> = backend_text_indices.iter()
            .map(|&i| {
                let c = &fallback_children[i];
                (c.relative_position.0, c.relative_position.1, c.width, c.height)
            })
            .collect();
        for child in fallback_children.iter_mut() {
            if child.kind != ChildKind::Element { continue; }
            let cx = child.relative_position.0;
            let cy = child.relative_position.1;
            let cw = child.width;
            let ch = child.height;
            let area = cw * ch;
            if area <= 0.0 { continue; }
            let max_overlap = ref_text.iter().map(|&(wx, wy, ww, wh)| {
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
            if max_overlap > 0.95 {
                child.kind = ChildKind::Text;
            }
        }
    }
    // Keep BFS components but discard original backend Text references by index
    let bfs_only: Vec<child::Child> = fallback_children.into_iter()
        .enumerate()
        .filter(|(i, _)| !backend_text_indices.contains(i))
        .map(|(_, c)| c)
        .collect();
    children.extend(bfs_only);

    // Pre-filter noise: remove children smaller than 0.25% of screen dim.
    let (_, _, w, h) = win_info.extents;
    let orig_len = children.len();
    children = filter::filter_tiny(children, w as f64, h as f64);
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
    // The raw BFS children produce 3-char hints, but only the visible
    // survivors remain after overlap filtering. Re-label those fresh.
    let kept = filter::cull_overlaps(&children, filter::overlap_limit(config.hints.hint_overlap_threshold));

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
                if backend_name == "imageproc" {
                    backend::imageproc::SAVE_DEBUG_IMAGES.store(config.dev.save_debug_images, std::sync::atomic::Ordering::Relaxed);
                }
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
            "drag" => {
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
                log::debug!("xdotool drag cmd: {} steps", steps);
                std::process::Command::new("sh")
                    .arg("-c").arg(&cmd)
                    .status()
                    .expect("Failed to spawn xdotool");
            }
            "select" => {
                let delay = (config.hints.drag_delay_ms as f64) / 1000.0;
                let cmd = format!("xdotool mousemove {} {}; sleep {}; xdotool mousedown {}; sleep {}; xdotool mousemove {} {}; sleep {}; xdotool mouseup {}",
                    action.x, action.y, delay, action.button, delay,
                    action.end_x, action.end_y, delay, action.button);
                log::debug!("xdotool select cmd: mousemove ({},{}) → ({},{})", action.x, action.y, action.end_x, action.end_y);
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