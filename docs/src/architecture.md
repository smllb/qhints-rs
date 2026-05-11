# Architecture

## Data flow

```
main.rs ──→ backends (scan window → Vec<Child>)
         ──→ hints.rs (assign labels → HashMap<label, index>)
         ──→ overlay GTK window (capture keyboard, draw hints, return action)
         ──→ main.rs (execute xdotool command)
```

### 1. Scan

The focused window's UI elements are detected through multiple backends:

1. **AT-SPI** runs first (async D-Bus tree walk, 250ms hard deadline)
2. **Fallback backends** run in configured order (imageproc, OCR)
3. All results merge into a single `Vec<Child>`

### 2. Filter

- Children smaller than 0.5% of screen dimensions are removed
- Pairwise overlap culling removes duplicates, preferring `Text` over `Element`
- Survivors are re-labeled with fresh short hints

### 3. Label

`hints::get_hints()` assigns keyboard labels using a spatial zone grid:

1. Screen divided into zones based on `first_key_zones` config
2. Children bucketed into their zone by position
3. Overflow redistributes to neighbors, then globally
4. Each zone gets single-char or multi-char labels

### 4. Show

A transparent GTK3 popup window is positioned over the target window.
Cairo renders hint labels, markers, and overlays. Keyboard is grabbed.

### 5. Input

Key events feed through a state machine that tracks:
- `typed` prefix buffer
- Current mode (normal, text selection, drag, double-click)
- Advanced mode sub-state
- Marker positions and offsets

### 6. Act

A `MouseAction` is returned to `main.rs` and dispatched via `xdotool`:
- **click**: `xdotool mousemove X Y click 1`
- **hover**: `xdotool mousemove X Y`
- **drag**: interpolated mousemove from source to destination
- **select**: mousedown at start, mousemove to end, mouseup

## Key data structures

```
Child {
    relative_position: (f64, f64),   // relative to window top-left
    absolute_position: (f64, f64),   // screen coordinates
    width: f64,
    height: f64,
    kind: ChildKind                  // Element | Text
}

MouseAction {
    action: String,                  // "click" | "hover" | "drag" | "select"
    x, y: i32,                       // primary coordinates
    end_x, end_y: i32,              // secondary coordinates (for drag/select)
    button: u32,                     // mouse button
    repeat: u32,                     // click count
    hunt_continue: bool,             // continue hunt loop?
    drag_fullscreen: bool            // trigger fullscreen re-scan?
}
```

## ChildKind system

- **`ChildKind::Text`** — word-level content from OCR or imageproc text detection
  - Gets blue border in text selection mode
  - Selection snaps to word edges
- **`ChildKind::Element`** — UI components (buttons, icons, BFS components)
  - Normal border always
  - Selection uses element center (left-to-right)

## Hint generation algorithm

```
first_key_zones (ragged grid: e.g. 10/9/7 columns)
        │
    map child (rx, ry) → (row, col) zone
        │
    per-zone capacity = len(zone_keys) × alphabet_len
        │
    overflow? → redistribute to neighbors (spatial)
      still overflow? → global redistribution
        │
    within capacity? → single-char keys
    within 2-char?   → first_key + alphabet_char
    else             → first_key + r1 + r2 (3-char)
```

Center-zone children get priority for shorter (single-char) labels.
Periphery zones prefer 2-char to reserve short labels for center elements.

## Overlap culling (two-pass)

**Pass 1 (main.rs):** Pairwise overlap on raw children before labeling
- When overlap exceeds threshold: `Text` wins over `Element`, otherwise the larger child survives

**Pass 2 (drawing.rs):** On rendered hint label rectangles
- Deterministic: keep the first (top-left) visible hint
- Uses `hint_overlap_threshold` config (default 60%)

## Threading

| Operation | Thread | Timeout |
|-----------|--------|---------|
| X11 init | Spawned thread | 2s |
| AT-SPI tree walk | Spawned thread with tokio | 250ms |
| imageproc scan | Spawned thread | 5s |
| OCR scan | Spawned thread | 15s |
| GTK main loop | Main thread | — |
| Pulse animation | GLib timer | marker_pulse_interval_ms |

**Note:** Spawned threads are NOT cancelled on timeout (Rust limitation). Internal
backends have their own timeouts, and the lock file prevents >2-3 orphaned threads.

## Lock file

A lock file at `/tmp/qhints.lock` prevents multiple concurrent instances.
Uses `flock(LOCK_EX | LOCK_NB)`.
