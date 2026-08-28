# Changelog

## [Unreleased]

### Changed
- Replaced the double max/min-of-RGB Canny with a **single fused contrast
  channel** (`max − min/2`): one Canny pass now covers both dark-on-light edges
  and bright saturated text (e.g. orange on white) that a plain max-of-RGB
  channel missed. Faster than the double-channel run while keeping the same
  detection.

### Removed
- `min_channel_edges` config field (no longer needed — the fused channel always
  recovers bright colored text).
- `MIN_CHANNEL_EDGES` benchmark environment toggle.

## [1.1.0] — 2026-08-13

### Added
- Screenshot benchmark test suite (`test-assets/screenshots/*.png`) with filename-encoded thresholds and `--update-baselines`.
- `min_channel_edges` configuration flag, which recovers bright colored text (e.g. orange on white) via a secondary min-of-RGB Canny pass.
- Benchmark pipeline-stage image outputs (`luma`, `edges`, `bfs_debug`, `annotated`) written to `target/benchmarks/`.

### Changed
- Parallelized the imageproc detection pipeline with `rayon`, distributing grayscale conversion, Canny edge detection (blur, Sobel, non-maximum suppression), edge combination, text projection, dilation, and connected-component labeling across CPU cores. Detection latency is reduced to approximately 35–45 ms per window.
- Independent `max`/`min` Canny passes now run on separate threads.
- `detection_scale` now supports downscaling (values below 1.0) for additional speed.
- The OCR backend reuses the parallel edge-detection and component helpers.

### Removed
- Unused evdev/enigo dependencies and the `mouse.rs` module.

## [text-selection] — 2026-05-08

### Added
- Text selection mode: press `/` (configurable `text_select_key`) to select text between two hints
- First hint marks selection start (red vertical marker on screen), second hint completes the range
- Text hints (words) select from left-edge to right-edge (full words); element hints select from center to center
- AT-SPI text role detection (`Label`, `Text`, `DocumentText`, `Static`, `Paragraph`, `Heading` → `ChildKind::Text`)
- OCR backend: produces text words with BFS gap-filling (BFS removed where OCR found text)
- Imageproc backend: BFS components + text line word detection
- All configured backends now run and merge results; overlap culling prefers `Text` children over `Element`
- Visual feedback: blue border on text hints in text selection mode
- Configurable `text_select_border_r/g/b/a` and `text_select_padding_left/right` (fraction of element width)
- `text_select_key` config option (default: `/` key, keyval 47)

### Changed
- `ChildKind` enum (`Element`, `Text`) added to distinguish word content from UI elements
- `MouseAction` gains `end_x`/`end_y` fields for the `"select"` action
- Backend loop no longer skips subsequent backends when children are found — all configured backends run
- Overlap culling in main.rs prefers `Text` children over `Element`
- 5s safety timeout respects text selection mode (keeps overlay alive between first and second hint)

## [feat/spotlight] — 2026-05-08

### Added
- Circular spotlight: dark overlay with radial gradient holes around matching hints on first keypress
- Configurable spotlight opacity (`spotlight_opacity`) and radius (`spotlight_radius`)/
- Hunt mode: continuous re-scan after each hint, Ctrl signals final selection, 10s idle dismiss
- Overlay is now mouse-transparent so underlying app keeps hover state

## [feat/stable-base] — Earlier

### Added
- Dynamic ragged-row grid: `first_key_zones` as `Vec<Vec<String>>`
- OCR backend (`ocrs`) behind feature flag
- Text line detection and word splitting in imageproc backend
- Per-side CSS-like `center_zone_padding`
- Global overflow redistribution for crowded zones
- Hunt mode idle timeout
- Dev grid overlay (`show_grid`)
- Config system with JSON merge over defaults
- AT-SPI D-Bus accessibility tree walker
- Imageproc fallback (Canny edge detection + BFS)
- X11 window system integration
- GTK overlay window with keyboard grab
