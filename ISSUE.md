# Issue: 3-key (3-character) hints appearing on screen periphery

## Branch
`feat/stable-base` (commit `354ead8`)

## Problem
The hint overlay shows 3-character hint labels (e.g., `"qas"`) on screen edges
(periphery — toolbars, sidebars, status bar). The goal is that periphery
elements always get 1-char or 2-char hints, and only the center text area
(middle of IDE text editor) may get 3-char if needed.

The user confirms the rendered hint COUNT is correct (~100 visible hints on
screen). The overlap culling in `drawing.rs` correctly filters visual noise.
But many of the surviving hints are 3-char, not 2-char.

## Constraints
1. Do NOT change the drawing overlap filtering (`drawing.rs`) — the rendered
   hint count is correct.
2. Do NOT reduce the number of visible hints.
3. Periphery (edges) should NEVER get 3-char hints.
4. Center (middle ~60% of screen) may get 3-char if needed.

## Config (runtime `~/.config/qhints/config.json`)
- `complementary_keys_alphabet`: `"asdfgqwerthlyuiopnm"` (18 chars)
- `first_key_zones`:
  ```
  ["qer", "df" , "g" ],
  ["as" , "th" , "ly"],
  ["ui" , "op" , "nm"]
  ```
- Zone keys per zone: 3,2,1 / 2,2,2 / 2,2,2 = 18 total
- Total 2-char capacity: 18 × 18 = **324**
- `center_zone_padding`: `{"top":0.1, "right":0.15, "bottom":0.05, "left":0.15}`
- `hint_overlap_threshold`: 60

## Backend: OCR + BFS (ocrs)
The OCR backend produces ~770–1080 raw children (text word detections + BFS
edge components for icons). After a minimal size pre-filter (0.5% of screen
dimensions), ~771 children remain.

## What We've Tried

### 1. Per-side padding (ZonePadding struct) — `src/config.rs`
ZonePadding with `top/right/bottom/left` fields. Periphery classified by
element position vs screen edges. Center zone classification uses zone
BOUNDARIES (not midpoint).

### 2. Global overflow redistribution — `src/hints.rs`
After neighbor-based overflow, a global pass distributes excess from any zone
to ANY zone with spare capacity (not just neighbors). Center zones process
first.

### 3. Periphery-first sorting — `src/hints.rs`
Within each zone, children sorted: periphery before center. Periphery gets
shorter labels (1-char if enough zone keys, else 2-char). Center gets
overflow (3-char if needed).

### 4. Cairo-based visible survivor detection — `src/main.rs`
Uses `gtk::cairo::ImageSurface` + `Context` to compute exact text extents
and replicate the drawing overlap culling. Finds survivors that would be
visible on screen, then re-labels only those with `get_hints`. Survivor
indices remapped to original `children` positions.

### 5. Pre-filter — `src/main.rs`
Minimum size filter (0.5% of smallest screen dimension) removes very small
noise children before hint generation.

## Debug Output (typical run with `-v`)
```
DEBUG qhints_rs] Filtered ~190 tiny children (now ~771)
DEBUG qhints_rs::hints] zone (0,0) periphery: 221 children, cap=54 (3 keys) → 167 will need 3-char!
DEBUG qhints_rs::hints] zone (0,1) periphery: 129 children, cap=36 (2 keys) → 93 will need 3-char!
DEBUG qhints_rs::hints] zone (1,0) periphery: 145 children, cap=36 (2 keys) → 109 will need 3-char!
...
DEBUG qhints_rs] Hint computation: ~250µs (~771 hints)
...
DEBUG qhints_rs::hints] zone (0,0) periphery: 140 children, cap=54 → 86 will need 3-char!
DEBUG qhints_rs] Re-labeled ~514 survivors from 771 raw (now 514 hints)
```

## Key Numbers
- Raw children after pre-filter: ~771
- Re-labeled survivors (cairo culling): ~514
- 2-char capacity: 324 (18 zone keys × 18 alphabet chars)
- **514 survivors > 324 capacity → 3-char overflow is unavoidable**

The cairo-based visible_survivors function apparently returns ~514 survivors,
which is still above the 2-char capacity. The user says they only see ~100
hints on screen, so there may be a discrepancy between `visible_survivors` and
the actual GTK rendering.

## Goal
Ensure zero 3-char hints on periphery edges while keeping the current
rendered hint count (~100). Center text area may still get 3-char if
mathematically necessary due to element density.

## Relevant Files
- `src/hints.rs` — hint generation, zone overflow redistribution, periphery-first sort
- `src/main.rs` — pre-filter, cairo-based visible_survivors, re-labeling
- `src/overlay/drawing.rs` — `visible_survivors()` function (cairo-based overlap culling)
- `src/overlay/mod.rs` — overlay keypress handler (300s safety net)
- `src/config.rs` — ZonePadding struct, per-side padding
