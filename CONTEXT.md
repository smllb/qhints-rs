# qhints-rs — Context

## What is this?

Keyboard-driven UI navigation for Linux (X11). Scans a focused window's UI elements, assigns keyboard labels (hints), and lets users click/hover/select elements by typing the hint keys. A Rust rewrite of [qhints](https://github.com/smllb/qhints).

## Architecture

```
main.rs → backends (scan window → Vec<Child>)
        → hints.rs (assign labels → HashMap<label, child_index>)
        → overlay GTK window (capture keyboard, draw hints, return MouseAction)
        → main.rs (execute xdotool command)
```

### Key files

| File | Role |
|---|---|
| `main.rs` | Entry point, CLI, hunt loop, backend orchestration, action dispatch |
| `overlay/mod.rs` | GTK popup overlay, keyboard grab, key event state machine, MouseAction |
| `overlay/drawing.rs` | Cairo rendering: hint boxes, colored borders, spotlight, grid, selection marker |
| `hints.rs` | Spatial zone-based hint label generation |
| `child.rs` | `Child` struct + `ChildKind` enum (`Element`, `Text`) |
| `config.rs` | Config structs, JSON merge loading |
| `backend/atspi.rs` | AT-SPI D-Bus accessibility tree walker |
| `backend/imageproc.rs` | Image-based detection (Canny + BFS + text line projection) |
| `backend/ocrs.rs` | OCR-based word detection + BFS gap-filling (feature-gated) |

### Data flow

1. **Scan**: AT-SPI runs first (async D-Bus tree walk). If no children found, configured backends run in order (`ocrs`, `imageproc`, etc.). All backends merge their results.
2. **Filter**: Tiny children (<0.5% screen) removed. Pairwise overlap culling removes duplicates, preferring `Text` children over `Element`.
3. **Label**: `hints.rs` assigns keyboard labels using a spatial zone grid.
4. **Show**: GTK overlay window renders hints with cairo. Keyboard is grabbed.
5. **Input**: Key events filtered through state machine (modes → exact match → prefix match).
6. **Act**: `MouseAction` returned to `main.rs`, dispatched via `xdotool`.

## Modes

### Normal mode
Type a hint → click at element center. Ctrl+hint → hover (no click). Alt (double-click mode) → double-click.

### Text selection mode
Press `/` (configurable) then type two hints. First hint marks selection start (red marker). Second hint completes the range. Text words select from word edge to word edge; elements from center to center.

### Double-click mode
Press Alt (configurable) then type one hint. That element gets double-clicked. Mode auto-resets after one use.

### Hunt mode
After an action, overlay re-appears for the next action. Ctrl during hunt signals "this is the last one".

## ChildKind system

- `ChildKind::Text` — word-level content from OCR or imageproc text detection. Gets blue border in text selection mode. Selection snaps to word edges.
- `ChildKind::Element` — UI components (buttons, icons, BFS components). Normal border always. Selection uses element center.

## Overlap culling

Two passes:
1. **main.rs** — Pairwise overlap culling on raw children before labeling. Prefers `Text` over `Element`, then keeps the larger child.
2. **drawing.rs** — Culling on rendered hint rects to avoid overlapping label boxes. Keeps the first (top-left) visible hint.

## Config

JSON file at `$XDG_CONFIG_HOME/qhints/config.json`. Merged over Rust defaults. Key options:

| Option | Default | Description |
|---|---|---|
| `backends` | `["atspi"]` | Ordered list of scanning backends |
| `exit_key` | 65307 (Esc) | Key to dismiss overlay |
| `hover_modifier` | 4 (Ctrl) | Held modifier for hover action |
| `double_click_key` | 65513 (Alt) | Toggle double-click mode |
| `text_select_key` | 47 (/) | Toggle text selection mode |
| `hints.text_select_padding_left/right` | 0.0 | Fraction of element width for selection offset |
| `hints.text_select_border_r/g/b/a` | blue | Border color in text selection mode |
| `dev.spotlight` | false | Dark overlay with holes around matching hints |
