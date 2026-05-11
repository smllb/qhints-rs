# Usage

Run the binary — a transparent overlay appears over the focused window with keyboard labels
on each UI element. Type a label to interact with that element.

```sh
./target/release/qhints-rs
```

## Options

| Flag | Description |
|------|-------------|
| `-m`, `--mode` | `hint` (default) or `scroll` |
| `-v` | Verbose logging (`-v` = debug, `-vv` = trace) |

## How it works

1. **Scan** — the focused window's UI elements are detected via AT-SPI (accessibility tree),
   image processing (Canny edge detection + BFS), or OCR.
2. **Label** — each element gets a keyboard hint (e.g. `qa`, `qs`, `qd`) based on its
   position on screen.
3. **Act** — type the hint to click at that element's center. Hold Ctrl on the last key
   to hover instead.

## Hunt mode

With `hunt: true` in config, the overlay re-appears after every action so you can chain
multiple operations. Hold Ctrl during hunt to signal "this is the last one".
Dismisses automatically after 10 seconds idle.
