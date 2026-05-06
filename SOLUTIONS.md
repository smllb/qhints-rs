# qhints-rs — Solutions Log

## Config & Naming

| Change | Branch | Description |
|--------|--------|-------------|
| XDG config path | `feat/config-and-docs` | Config reads from `$XDG_CONFIG_HOME/qhints/config.json`, fallback `~/.config/qhints/config.json` |
| `first_key_zones` | `feat/rename-fields` | Renamed from `keyboard_zones` — first character per screen zone |
| `complementary_keys_alphabet` | `feat/rename-fields` | Renamed from `alphabet` — characters for second+ position in multi-char hints |
| Deduplicated zone keys | `feat/rename-fields` | Every first-key char must be unique across all 9 zones to avoid HashMap collisions |
| `center_zone_padding` | `feat/text-line-recognition` | Periphery elements get 2-char priority; center (text) can take 3-char. Configurable per-app. |
| Canny thresholds lowered | `feat/ocr-backend` | Default `canny_min_val` 15 / `canny_max_val` 40 for thinner box detection |

## Hint Generation

| Change | Branch | Description |
|--------|--------|-------------|
| Zone overflow → sequential 2-char | `feat/no-3key-chords` | Replaced 3-char fallback with sequential 2-char (reverted due to collisions on crowded screens) |
| 3-char restored for initial gen | `feat/no-3key-chords` | Initial gen allows 3-char; overlay re-hint still caps at 2-char (re-hinting later removed) |
| No re-hinting in overlay | `feat/text-line-recognition` | Labels are stable after prefix filter — no mid-flight label changes |
| `center_zone_padding` | `feat/text-line-recognition` | During 3-char fallback, periphery children get 2-char first, center gets overflow 3-char |

## Imageproc Backend

| Change | Branch | Description |
|--------|--------|-------------|
| BFS + word box overlap filter | `feat/text-line-recognition` | BFS components whose center falls inside a word box → removed (keeps one hint per word) |
| Area-based overlap check | `feat/text-line-recognition` | Changed from center-in-box to area-overlap fraction for more accurate filtering |
| Min space width for word gaps | `feat/text-line-recognition` | Requires 3+ consecutive gap columns to split a word (avoids character-internal splits) |

## OCR Backend (ocrs)

| Change | Branch | Description |
|--------|--------|-------------|
| Feature-gated OCR backend | `feat/ocr-backend` | `cargo build --features ocr` enables ocrs 0.12 + rten 0.24 + ureq 3 |
| Model auto-download | `feat/ocr-backend` | Downloads detection + recognition models to `~/.cache/qhints/ocrs/` on first run |
| Config-driven fallbacks | `feat/ocr-backend` | `main.rs` iterates `config.backends` in order (atspi → ocrs → imageproc) |
| OCR + BFS merge | `feat/ocr-backend` | OCR returns text word boxes; BFS catches icons/buttons/non-text; combined in one backend pass |
| No overlap filtering | `feat/ocr-backend` | All BFS components kept alongside OCR words — overlay's overlap culling handles duplicates |

## Text Line Detection (imageproc)

| Change | Branch | Description |
|--------|--------|-------------|
| Horizontal projection | `feat/text-line-recognition` | Row-sum threshold detects line bands from edge image |
| Vertical projection per line | `feat/text-line-recognition` | Column-sum threshold splits line into word segments |
| `gap_ratio` = 0.25 | `feat/text-line-recognition` | Column needs <25% of line-height edge pixels to be a gap |
