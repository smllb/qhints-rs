mod backend;
mod child;
mod config;
mod hints;
mod mouse;
mod overlay;
mod window_system;

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

    // Imageproc fallback (with 5s hard timeout)
    if children.is_empty() {
        log::debug!("AT-SPI found no children. Falling back to Imageproc.");
        let cv_start = Instant::now();

        let (w, h) = (win_info.extents.2, win_info.extents.3);
        if w as u64 * h as u64 > 1920 * 1080 {
            log::warn!("Large image for imageproc: {}x{} — may block briefly", w, h);
        }

        let win_info_clone = win_info.clone();
        let rule_clone = rule.clone();
        children = with_thread_timeout(
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
        });
        log::debug!("Imageproc fallback: {:?} ({} children)", cv_start.elapsed(), children.len());
    }

    if children.is_empty() {
        log::debug!("No accessible children found");
        return;
    }

    // Compute hints
    let t = Instant::now();
    let (_, _, w, h) = win_info.extents;
    let hint_map = hints::get_hints(&children, &config.alphabet, Some((w as f64, h as f64)));
    log::debug!("Hint computation: {:?} ({} hints)", t.elapsed(), hint_map.len());

    // Show overlay
    let (x, y, width, height) = win_info.extents;
    if let Some(action) = overlay::show_overlay(config, &hint_map, &children, x, y, width, height) {
        log::debug!("Action: {:?}", action);

        match action.action.as_str() {
            "click" => {
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(format!(
                        "xdotool mousemove {} {} click {}",
                        action.x, action.y, action.button
                    ))
                    .spawn()
                    .expect("Failed to spawn xdotool");
            }
            "hover" => {
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(format!("xdotool mousemove {} {}", action.x, action.y))
                    .spawn()
                    .expect("Failed to spawn xdotool");
            }
            _ => {
                log::debug!("Unhandled action: {}", action.action);
            }
        }
    }
    // _lock drops here, releasing the flock
}