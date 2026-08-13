# Screenshot benchmarks

Drop PNG screenshots of app windows here. Each PNG is run through the
imageproc detection pipeline on every `cargo test`, and the **final hint
count** (the labels a user would actually see, after tiny-filter + overlap
culling) is checked against the range encoded in the filename:

| Filename | Meaning |
|---|---|
| `30_min.png` | at least 30 hints |
| `30_min_60_max.png` | between 30 and 60 hints |
| `anything_10_min.png` | at least 10 hints (default min is 30) |

The `min`/`max` numbers must appear immediately before the `min`/`max`
keywords (underscores/dashes allowed between number and keyword).

Annotated results and a `report.csv` are written to `target/benchmarks/` (git-ignored).

## Reading the annotated images

Each annotated PNG draws the children found by the imageproc pipeline, color-coded by what the user would actually see:

| Color | Meaning |
|---|---|
| **Green** | `Element` hint — a UI component (button, icon, BFS component). These survive culling and get a label. |
| **Blue** | `Text` hint — a word detected by text-line projection. Survives culling and gets a label. |
| **Red** | Culled — a child removed by overlap culling (duplicate detection), so it gets **no** label. |

Green + blue = the hints a user would type. Red = candidates that were thrown away.
