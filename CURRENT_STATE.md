# Current State

## Architecture

```
main.rs → backends (scan window → Vec<Child>)
        → hints.rs (spatial zone labels → HashMap<label, index>)
        → overlay GTK window (keyboard grab, draw hints, return MouseAction)
        → main.rs (execute xdotool)
```

## Backends

| Backend | Type | Description |
|---------|------|-------------|
| `atspi` | Primary | Async D-Bus accessibility tree walk. Text roles → `Text`, rest → `Element`. |
| `ocrs` (optional) | Fallback | OCR word detection + BFS edge components. Words → `Text`, BFS → `Element`. BFS overlapping words >30% filtered. |
| `imageproc` | Fallback | Canny edge → BFS components + text line projection. BFS → `Element`, text lines → `Text`. BFS overlapping text >30% filtered. |

All configured backends run and merge results. Order: `["atspi", "ocrs", "imageproc"]`.

## Modes

| Key | Mode | Basic | Advanced |
|-----|------|-------|----------|
| Type hint keys | Normal | Type label → click | — |
| Ctrl on last key | Hover | Hold Ctrl → type label → mousemove only | — |
| Alt (toggle) | Double-click | Press Alt → type label → double-clicks | — |
| / (toggle) | Text selection | Press / → pick start → pick end → selects | After start, / or Ctrl → pick end (marker) → arrows nudge → Tab switch → Enter confirm |
| Shift (toggle) | Drag | Press Shift → pick source → pick dest → executes | After source, Ctrl → pick dest (marker) → arrows nudge → Tab switch → Enter confirm. Or Shift → fullscreen scan → pick dest anywhere |

## Overlap culling

Two-pass:
1. **main.rs**: Pairwise overlap. Text always wins over Element when overlap exceeds threshold (default >40%). Otherwise keeps larger child.
2. **drawing.rs**: On rendered hint rects, keeps first (top-left) visible hint when boxes overlap.

## Visual feedback

- **Blue border**: Text hints in text selection mode
- **Green border**: All hints in drag mode
- **Thicker border**: Double-click mode; advanced mode (configurable extra width)
- **Pulsing markers**: All markers pulse gently. Active/adjusted marker pulses more intensely. Bright flash on placement/Tab switch.
- **Red vertical line**: Text selection start marker
- **Orange vertical line**: Text selection end marker (advanced mode)
- **Filled circles**: Drag source (red) and destination (green) — configurable size/shape
- **Spotlight rectangle**: Between start/end in advanced text selection
- **show_text_boxes (dev)**: Blue bounding boxes around detected Text words

## Config highlights

| Field | Default | Description |
|-------|---------|-------------|
| `exit_key` | 65307 | Escape |
| `hover_modifier` | 4 | Ctrl mask |
| `double_click_key` | 65513 | Alt |
| `text_select_key` | 47 | / |
| `drag_key` | 65505 | Shift |
| `advanced_modifier` | 0 | Global key for advanced mode (e.g. 65507 for Ctrl). 0 = per-mode defaults |
| `backends` | ["atspi"] | Scan backends |
| `hints` | — | Label appearance + behavior |
| `dev.show_text_boxes` | false | Debug: show Text word bounding boxes |

## Threading

- AT-SPI: async D-Bus via tokio, 250ms hard deadline
- Backends: `with_thread_timeout` (5s imageproc, 15s ocrs)
- Overlay: GTK main loop (blocking)
- Pulse animation: 30-83ms interval timeout

## Known issues

1. **Fullscreen overlay on i3wm**: override_redirect window may not cover all monitors
2. **GLM-OCR**: base64 encoding and shell argument limits make large screenshots impractical
3. **AT-SPI**: Most apps (VS Code, browsers) don't expose accessibility without explicit flags
