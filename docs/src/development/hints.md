# src/hints.rs

## Public functions

```rust
pub fn get_hints(
    children: &[Child],
    complementary_keys_alphabet: &str,
    first_key_zones: &[Vec<String>],
    center_zone_padding: &config::ZonePadding,
    window_size: Option<(f64, f64)>,
) -> HashMap<String, usize>
```

Returns a map of hint label string → child index.

## Private functions

```rust
fn get_zone(
    rx: f64,
    ry: f64,
    width: f64,
    height: f64,
    rows: usize,
    col_counts: &[usize],
) -> (usize, usize)
```

Normalizes `(rx, ry)` by `(width, height)` and maps to ragged grid `(row, col)`.

```rust
fn neighbors(
    r: usize,
    c: usize,
    rows: usize,
    col_counts: &[usize],
) -> Vec<(usize, usize)>
```

Returns all 8-directional adjacent zones that exist (respecting ragged rows),
sorted by Euclidean distance from `(r, c)`.

```rust
fn generate_product(
    chars: &[char],
    repeat: usize,
    out: &mut Vec<String>,
)
```

Recursive cartesian product. Generates all possible `String`s of length `repeat`
from `chars`. Used as fallback when no window_size is available.
