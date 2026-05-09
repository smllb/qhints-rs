# Changelog

## [Unreleased] — feat/double-click

### Added
- Double-click mode: press Alt (configurable `double_click_key`) then a hint to double-click
- Visual feedback: thicker borders when double-click mode is active
- Mutual exclusivity between double-click and text selection modes

### Changed
- `grab_modifier` replaced with `double_click_key`
- `repeat` field now wired to xdotool for proper multi-click

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
