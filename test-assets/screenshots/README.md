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

Annotated results (green = Element hint, blue = Text hint, red = culled)
and a `report.csv` are written to `target/benchmarks/` (git-ignored).
