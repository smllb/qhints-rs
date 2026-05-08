# Backends

Scan a window and return `Vec<Child>`. Configured via `backends` in config.json. All configured backends run and their results are merged.

## atspi
AT-SPI D-Bus accessibility tree walk. Async, batched. Identifies text roles (`Label`, `Text`, `DocumentText`, etc.) as `ChildKind::Text`, everything else as `Element`.

## imageproc
Screenshot → Canny edge detection → dilate → BFS connected components → text line/word detection via horizontal/vertical projection. BFS components are `Element`, text lines are `Text`.

## ocrs (feature-gated)
OCR engine for text detection. Produces word bounding boxes as `ChildKind::Text`, then runs BFS to fill gaps (BFS components overlapping words removed).
