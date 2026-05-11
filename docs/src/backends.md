# Backends

Backends are configured via the `backends` config field. All configured backends
run in order and their results are merged.

Default: `["atspi"]`

```json
{ "backends": ["atspi", "imageproc", "ocrs"] }
```

## Merge pipeline

```
AT-SPI ──────┐
imageproc ───┤── merge ──→ filter tiny ──→ overlap cull ──→ label
ocrs ────────┘
```

When multiple backends produce overlapping children:
1. Text references from fallback backends reclassify BFS components (`Element` → `Text`)
   when overlap exceeds 95%
2. Original backend Text references are discarded — only BFS components survive
3. Pairwise overlap culling prefers `Text` over `Element`

## AT-SPI (async D-Bus)

Connects to the system's accessibility bus via `atspi` + `zbus`. Walks the
accessibility tree of the focused window.

- Queries roles, states, and geometry via batched async calls
- Max recursion depth: 20
- Max children per level: 500
- Filters by application state (Sensitive + Showing + Visible)
- Excludes roles: PANEL, FRAME, MENU_BAR, TOOL_BAR, LIST, SCROLL_PANE, TABLE, etc.
- **Timeout:** 150ms tokio timeout + 250ms hard deadline

### Text role detection

The following AT-SPI roles produce `ChildKind::Text`:
Label(36), Text(74), DocumentText(87), Static(116), Paragraph(73), Heading(83)

Everything else becomes `ChildKind::Element`.

### Requirements

```sh
systemctl --user status at-spi-dbus-bus.service
```

Some applications (VS Code, browsers) may not expose accessibility info unless
launched with appropriate flags.

## Imageproc (computer vision)

Screenshot-based detection. Runs when `"imageproc"` is in the backends list.

### Pipeline

1. **Screenshot** — X11 `GetImage` on the window region
2. **Grayscale conversion** — dual pass: max-of-RGB (for edges) and
   weighted luminance (for text detection)
3. **Canny edge detection** — configurable min/max thresholds and detection scale
4. **Text word detection** — horizontal projection → text line bands →
   vertical projection per line → word segments
5. **Dilation** — morphological dilation to connect nearby edges
6. **BFS** — 4-direction flood fill → connected components
7. **Filter** — remove components >50% of window
8. **Overlap analysis** — BFS components overlapping text words by >95% are
   reclassified as `Text`

### Output

- BFS connected components → `ChildKind::Element`
- Text line/word segments → `ChildKind::Text`
- Debug images saved to `/tmp/qhints_debug/` when `dev.save_debug_images` is enabled

### Per-app tuning

```json
{
  "application_rules": {
    "firefox": {
      "canny_min_val": 20,
      "canny_max_val": 50,
      "kernel_size": 3,
      "detection_scale": 1.0
    }
  }
}
```

Higher `canny_min_val` = fewer edges (sparser detection).
Higher `detection_scale` = upscale before detection (more detail, slower).

**Timeout:** 5 seconds.

## OCR (optional)

Feature-gated (`default = ["ocr"]`). Disable with `--no-default-features`.

Uses `ocrs` + `rten` for text detection and recognition. Downloads pre-trained
models (~35MB) from AWS S3 on first run to `~/.cache/qhints/ocrs/`.

### Pipeline

1. **Screenshot** — same X11 `GetImage` as imageproc
2. **OCR** — `ocrs::OcrEngine` detects word bounding boxes
3. **Word boxes** → `ChildKind::Text`
4. **BFS gap-filling** — Canny edge detection + dilation + BFS on the same
   screenshot to find non-text elements
5. **Filter** — BFS components overlapping OCR words by >30% are removed
6. **Merge** — BFS components appended first, then word boxes

### Models

- `text-detection.rten` — finds text regions
- `text-recognition.rten` — recognizes characters (not used for positioning)

**Timeout:** 15 seconds.
