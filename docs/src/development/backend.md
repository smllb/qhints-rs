# src/backend/

## src/backend/mod.rs

```rust
pub mod atspi;
pub mod imageproc;
#[cfg(feature = "ocr")]
pub mod ocrs;
```

## src/backend/atspi.rs

Async D-Bus accessibility tree walker.

```rust
pub struct AtspiBackend {
    conn: Connection,
    rule: ApplicationRule,
    window_info: WindowInfo,
    scale_factor: f64,
}

impl AtspiBackend {
    pub async fn new(
        window_info: WindowInfo,
        rule: ApplicationRule,
    ) -> Result<Self, Box<dyn std::error::Error>>

    pub async fn get_children(
        &self,
    ) -> Result<Vec<Child>, Box<dyn std::error::Error>>
}
```

### `get_children()` flow

1. `find_active_window()` — iterate D-Bus tree for app → window matching PID + Active state
2. `walk_children(proxy, &mut children, 0)` — recursive, max depth 20, max 500/level
   - Batch-fetch children via `futures::join_all`
   - Query role, state, extents via `tokio::join!`
   - Filter: ALL of (Sensitive, Showing, Visible) AND NONE of EXCLUDED_ROLES
   - Role → `ChildKind::Text` if `is_text_role()`, else `Element`
   - Recurse via `Box::pin(self.walk_children(...))`

### Private methods

```rust
async fn find_active_window(
    &self,
) -> Result<Option<AccessibleProxy<'_>>, zbus::Error>

async fn walk_children(
    &self,
    proxy: &AccessibleProxy<'_>,
    children: &mut Vec<Child>,
    depth: usize,
) -> Result<(), Box<dyn std::error::Error>>

fn is_text_role(role: i32) -> bool
```

### `is_text_role()` matches

| Role ID | Name |
|---------|------|
| 36 | Label |
| 74 | Text |
| 87 | DocumentText |
| 116 | Static |
| 73 | Paragraph |
| 83 | Heading |

## src/backend/imageproc.rs

Computer vision: screenshot → edge detection → BFS → text lines.

### Statics

```rust
pub static DEBUG_BFS_COMPONENTS: Mutex<Vec<Child>>
pub static SAVE_DEBUG_IMAGES: AtomicBool
```

### Public functions

```rust
pub fn get_children(
    window_info: &WindowInfo,
    rule: &ApplicationRule,
) -> Result<Vec<Child>, Box<dyn std::error::Error>>
```

### Pipeline

1. **Screenshot**: X11 `GetImage(Z_PIXMAP, root, x, y, w, h)` → BGRA buffer
2. **Dual grayscale** (single pass over BGRA):
   - `luma`: `0.299R + 0.587G + 0.114B` — debug / text-height estimate
   - `fused`: `max(R, G, B) − min/2` — single contrast channel preserving dark and
     bright saturated edges
3. **Canny edge detection** on the fused channel:
   - Resize by `detection_scale` (Nearest neighbor)
   - Parallel Canny (`canny_min_val`, `canny_max_val`)
4. **Dilation**: `dilate_parallel(edges, kernel_size/2)` — bridges gaps in strokes
5. **Connected components** (parallel run-based CC, with pixel area) → raw "pieces"
6. **Filter**: remove pieces <1px and >50% of the window (containers)
7. **Estimate text height**: sliding-mode of piece heights within `text_height_min..max`
   (used only for the debug `text_h` display)
8. **Hints** are assigned directly to the pieces (all `ChildKind::Element`)
9. **Debug images**: Saved to `/tmp/qhints_debug/` if `SAVE_DEBUG_IMAGES`

### Private functions

```rust
fn draw_boxes(
    luma: &GrayImage,
    pieces: &[Child],
    children: &[Child],
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>>
```

## src/backend/ocrs.rs (feature-gated)

OCR-based detection via `ocrs` + `rten`.

### Constants

```rust
const DETECTION_MODEL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
const RECOGNITION_MODEL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";
```

### Public functions

```rust
pub fn get_children(
    window_info: &WindowInfo,
    rule: &ApplicationRule,
) -> Result<Vec<Child>, Box<dyn std::error::Error>>
```

### Pipeline

1. **Screenshot**: X11 `GetImage` (same as imageproc)
2. **RGB conversion**: BGRA → flat RGB array; also build luma for BFS
3. **Model download**: Download models to `~/.cache/qhints/ocrs/`
   - Skips if already cached
4. **OCR**: `ocrs::OcrEngine::new` with detection + recognition models
   - `engine.prepare_input(ImageSource)` → `ocr_input`
   - `engine.detect_words(&ocr_input)` → word bounding boxes as `ChildKind::Text`
5. **BFS gap-filling**: Canny + dilation + BFS on luma → `ChildKind::Element`
6. **Filter**: Remove BFS components overlapping OCR words >30%
7. **Merge**: BFS first, then word boxes appended

### Private functions

```rust
fn cache_dir() -> Result<PathBuf, Box<dyn std::error::Error>>

fn download_model(
    url: &str,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>>
```
