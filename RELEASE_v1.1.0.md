# qhints-rs v1.1.0

Keyboard-driven UI navigation for Linux (X11). Scans a focused window, assigns keyboard labels ("hints"), and lets the user click, hover, drag, or select elements by typing those labels.

This release focuses on detection performance and improved recovery of colored text.

---

## Highlights

- **Approximately 30× faster detection.** The imageproc detection pipeline (grayscale conversion, Canny edge detection, text projection, dilation, and connected-component labeling) is now parallelized with [rayon](https://github.com/rayon-rs/rayon), distributing work across all available CPU cores. Detection latency is reduced from approximately one second to roughly 35–45 ms per window.
- **Colored-text recovery.** A secondary Canny pass on the `min(R,G,B)` channel — executed in parallel with the `max` pass — recovers bright colored text, such as orange text on a white background, that the previous single-channel detector missed. Controlled by the `min_channel_edges` option.
- **Downscaling.** `detection_scale` now accepts values below 1.0, permitting a trade-off between precision and speed.
- **Screenshot benchmark suite.** Screenshots may be placed in `test-assets/screenshots/`, named with thresholds (e.g. `30_min_60_max.png`), and `cargo test` asserts the resulting hint count. `UPDATE_BASELINES=1` records the current values as baselines.

## Changes since v1.0.0

### Performance
- Parallelized imageproc pipeline (grayscale → Canny → combination → projection → dilation → components).
- Independent `max`/`min` Canny passes on separate threads.
- Parallel connected-component labeling (run-length encoding with union-find).
- OCR backend reuses the parallel helpers.

### Detection quality
- `min_channel_edges` recovers bright colored text (enabled by default).
- `detection_scale` downscaling (0.1–4.0) for the speed/quality trade-off.

### Tooling
- Screenshot benchmark tests with filename-encoded thresholds and `--update-baselines`.
- Benchmark pipeline-stage image outputs (`luma`, `edges`, `bfs_debug`, `annotated`).

## Modes

| Mode | Key | Behavior |
|------|-----|----------|
| Normal | hint keys | Click element |
| Hover | Ctrl + hint | Move mouse without clicking |
| Double-click | Alt | Toggle then type hint |
| Text select | `/` | Pick start → pick end to select text |
| Drag | Shift | Pick source → pick destination to drag |
| Hunt | dev option | Re-scan after every action, Ctrl to quit |

## Requirements

- X11 session
- `xdotool`, `libgtk-3-0`
- AT-SPI D-Bus service (`at-spi-dbus-bus.service`)
- Rust 1.87+ (to build from source)

## Install

Download the binary and make it executable:

```
chmod +x qhints-rs
sudo mv qhints-rs /usr/local/bin/
```

Or build from source:

```
cargo build --release
# with OCR: cargo build --release --features ocr
```

---

[GitHub](https://github.com/smllb/qhints-rs) · [Documentation](https://smllb.github.io/qhints-rs/)
