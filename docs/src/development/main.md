# src/main.rs

## Module declarations

```rust
mod backend;
mod child;
mod config;
mod hints;
mod overlay;
mod window_system;
```

## CLI

```rust
#[derive(Parser)]
#[command(name = "qhints-rs", about = "Keyboard-driven UI navigation for Linux")]
struct Cli {
    #[arg(short, long, default_value = "hint")]
    mode: String,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}
```

## Public functions

```rust
fn main()
```

1. Parse CLI args
2. Set log level: 0=Info, 1=Debug, 2+=Trace
3. Load config
4. Dispatch: `"hint"` → `hint_mode()`, `"scroll"` → warn not implemented

## Private functions

```rust
fn try_acquire_lock() -> Option<std::fs::File>
```

Creates `/tmp/qhints.lock` and tries `flock(LOCK_EX | LOCK_NB)`. Returns `Some(file)`
on success, `None` if another instance holds the lock.

```rust
fn with_thread_timeout<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
    timeout: std::time::Duration,
    label: &'static str,
) -> Option<T>
```

Spawns a thread for `f()`, waits up to `timeout` for a result via `mpsc::channel`.
Returns `Some(T)` on success, `None` on timeout or thread panic.

**Thread is not cancelled on timeout.** Backends have internal timeouts and the
lock file prevents >2-3 orphaned threads.

```rust
fn hint_mode(config: &config::Config, total_start: Instant)
```

### Flow

1. **Lock**: `try_acquire_lock()` — exit if already running
2. **X11 init**: `X11::new()` via `with_thread_timeout(2s)`
3. **App rule**: Lookup `ApplicationRule` by `win_info.app_name`, fallback to `"default"`

### Hunt loop (repeat until done)

#### A. AT-SPI scan (async threaded)
```
tokio::time::timeout(150ms, async { AtspiBackend::new(...).get_children().await })
  → rx.recv_timeout(250ms)
  → Vec<Child> or empty
```

#### B. Fallback backends (in config order, skip "atspi")
- `"imageproc"`: `with_thread_timeout(|| imageproc::get_children(), 5s)`
- `"ocrs"`: `with_thread_timeout(|| ocrs::get_children(), 15s)`

#### C. Merge fallback text references
1. Find `Text` indices in `fallback_children`
2. Reclassify `Element` → `Text` where overlap > 95%
3. Discard original backend `Text` references, keep only BFS

#### D. Filter tiny children
Remove children < 0.5% of screen (min 3px on each axis)

#### E. Initial labeling
```
hints::get_hints(&children, &alphabet, &zones, &padding, Some((w, h)))
```

#### F. Overlap culling & relabeling
Threshold: `(100 - hint_overlap_threshold) / 100`

Pairwise comparison:
- `Text` wins over `Element`
- Otherwise smaller child is culled

If any culled → re-label survivors with fresh short hints

#### G. Show overlay
```
overlay::show_overlay(config, &hint_map, &children, x, y, w, h, None)
  → Option<MouseAction>
```

#### H. Dispatch action

| `action.action` | xdotool command |
|---|---|
| `"click"` | `mousemove X Y [click BUTTON] × repeat` |
| `"hover"` | `mousemove X Y` |
| `"drag"` | `mousemove SX SY; sleep; mousedown; steps(8px); sleep; mouseup` |
| `"select"` | `mousemove SX SY; sleep; mousedown; mousemove EX EY; sleep; mouseup` |

If `drag_fullscreen`:
1. Get screen size
2. Run all non-atspi backends on full screen
3. Show second overlay with `preset_drag_source`
4. Execute drag with interpolation

If `hunt_continue`: `sleep(hunt_timeout_ms)`, continue loop
Otherwise: `break`
