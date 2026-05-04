use crate::child::Child;
use crate::config::KEYBOARD_ZONES;
use std::collections::HashMap;

/// Map a child's relative position to a 3x3 screen zone.
fn get_zone(rx: f64, ry: f64, width: f64, height: f64) -> (usize, usize) {
    let nx = if width > 0.0 {
        (rx / width).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let ny = if height > 0.0 {
        (ry / height).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let col = ((nx * 3.0) as usize).min(2);
    let row = ((ny * 3.0) as usize).min(2);
    (row, col)
}

/// Adjacent zones sorted by grid distance (cardinal first).
fn neighbors(r: usize, c: usize) -> Vec<(usize, usize)> {
    let mut nbrs = Vec::new();
    for dr in [-1i32, 0, 1] {
        for dc in [-1i32, 0, 1] {
            if dr == 0 && dc == 0 {
                continue;
            }
            let nr = r as i32 + dr;
            let nc = c as i32 + dc;
            if (0..=2).contains(&nr) && (0..=2).contains(&nc) {
                nbrs.push((nr as usize, nc as usize));
            }
        }
    }
    nbrs.sort_by_key(|&(nr, nc)| {
        let dr = nr as i32 - r as i32;
        let dc = nc as i32 - c as i32;
        dr * dr + dc * dc
    });
    nbrs
}

/// Generate hints with spatial zone-based keyboard assignment.
///
/// Port of Python `get_hints()` — assigns hint labels based on
/// the child's screen position mapped to keyboard zones.
pub fn get_hints(
    children: &[Child],
    alphabet: &str,
    window_size: Option<(f64, f64)>,
) -> HashMap<String, usize> {
    let mut hints: HashMap<String, usize> = HashMap::new();

    if children.is_empty() {
        return hints;
    }

    let alpha_chars: Vec<char> = alphabet.chars().collect();

    // Fall back to sequential assignment when spatial mapping isn't possible.
    let (width, height) = match window_size {
        Some(size) => size,
        None => {
            let n_chars =
                (children.len() as f64).ln().ceil() / (alpha_chars.len() as f64).ln().ceil();
            let n_chars = (n_chars as usize).max(1);
            let mut labels = Vec::new();
            generate_product(&alpha_chars, n_chars, &mut labels);
            for (i, label) in labels.into_iter().enumerate() {
                if i >= children.len() {
                    break;
                }
                hints.insert(label, i);
            }
            return hints;
        }
    };

    // Bucket children into their 3x3 screen zone.
    let mut zone_buckets: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (i, child) in children.iter().enumerate() {
        let (rx, ry) = child.relative_position;
        let zone = get_zone(rx, ry, width, height);
        zone_buckets.entry(zone).or_default().push(i);
    }

    // Redistribute overflow
    let zone_cap = |r: usize, c: usize| -> usize {
        KEYBOARD_ZONES[r][c].len() * alpha_chars.len()
    };
    let zone_center_px = |r: usize, c: usize| -> (f64, f64) {
        ((c as f64 + 0.5) / 3.0 * width, (r as f64 + 0.5) / 3.0 * height)
    };

    for _ in 0..9 {
        let mut moved_any = false;
        let zones: Vec<(usize, usize)> = zone_buckets.keys().copied().collect();
        for zone in zones {
            let cap = zone_cap(zone.0, zone.1);
            let bucket_len = zone_buckets.get(&zone).map_or(0, |b| b.len());
            if bucket_len <= cap {
                continue;
            }
            let mut excess = bucket_len - cap;
            for nbr in neighbors(zone.0, zone.1) {
                let nbr_cap = zone_cap(nbr.0, nbr.1);
                let nbr_len = zone_buckets.get(&nbr).map_or(0, |b| b.len());
                let space = nbr_cap.saturating_sub(nbr_len);
                if space == 0 {
                    continue;
                }
                let (ncx, ncy) = zone_center_px(nbr.0, nbr.1);

                // Sort by distance to neighbor center
                if let Some(bucket) = zone_buckets.get_mut(&zone) {
                    bucket.sort_by(|&a, &b| {
                        let (ax, ay) = children[a].relative_position;
                        let (bx, by) = children[b].relative_position;
                        let da = (ax - ncx).powi(2) + (ay - ncy).powi(2);
                        let db = (bx - ncx).powi(2) + (by - ncy).powi(2);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }

                let to_move = space.min(excess);
                let moved: Vec<usize> = zone_buckets
                    .get_mut(&zone)
                    .map(|b| b.drain(..to_move).collect())
                    .unwrap_or_default();
                zone_buckets.entry(nbr).or_default().extend(moved);
                excess -= to_move;
                moved_any = true;
                if excess == 0 {
                    break;
                }
            }
        }
        if !moved_any {
            break;
        }
    }

    // Sort each bucket top-to-bottom, left-to-right
    for bucket in zone_buckets.values_mut() {
        bucket.sort_by(|&a, &b| {
            let (ax, ay) = children[a].relative_position;
            let (bx, by) = children[b].relative_position;
            ay.partial_cmp(&by)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal))
        });
    }

    // Assign hints per zone
    for (&(row, col), zone_children) in &zone_buckets {
        let zone_keys: Vec<char> = KEYBOARD_ZONES[row][col].chars().collect();
        let n = zone_children.len();

        if n <= zone_keys.len() {
            // Single-char hints
            for (child_idx, &key) in zone_children.iter().zip(zone_keys.iter()) {
                hints.insert(key.to_string(), *child_idx);
            }
        } else {
            // Multi-char: first char = zone key, rest = full alphabet
            let mut labels = Vec::new();
            'outer: for &first in &zone_keys {
                for &rest in &alpha_chars {
                    labels.push(format!("{}{}", first, rest));
                    if labels.len() >= n {
                        break 'outer;
                    }
                }
            }

            // 3-char fallback if still not enough
            if labels.len() < n {
                labels.clear();
                'outer3: for &first in &zone_keys {
                    for &r1 in &alpha_chars {
                        for &r2 in &alpha_chars {
                            labels.push(format!("{}{}{}", first, r1, r2));
                            if labels.len() >= n {
                                break 'outer3;
                            }
                        }
                    }
                }
            }

            for (child_idx, label) in zone_children.iter().zip(labels.into_iter()) {
                hints.insert(label, *child_idx);
            }
        }
    }

    hints
}

/// Generate cartesian product of chars with given repeat count.
fn generate_product(chars: &[char], repeat: usize, out: &mut Vec<String>) {
    if repeat == 0 {
        out.push(String::new());
        return;
    }
    let mut sub = Vec::new();
    generate_product(chars, repeat - 1, &mut sub);
    for c in chars {
        for s in &sub {
            out.push(format!("{}{}", c, s));
        }
    }
}
