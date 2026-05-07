use crate::child::Child;
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
    complementary_keys_alphabet: &str,
    first_key_zones: &[[String; 3]; 3],
    center_zone_padding: &crate::config::ZonePadding,
    window_size: Option<(f64, f64)>,
) -> HashMap<String, usize> {
    let mut hints: HashMap<String, usize> = HashMap::new();

    if children.is_empty() {
        return hints;
    }

    let alpha_chars: Vec<char> = complementary_keys_alphabet.chars().collect();

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

    // Redistribute overflow — periphery zones get priority to spill into
    // center zones, so periphery never needs 3-char.
    let zone_cap = |r: usize, c: usize| -> usize {
        first_key_zones[r][c].len() * alpha_chars.len()
    };
    let is_center_zone = |r: usize, c: usize| -> bool {
        let zx1 = c as f64 / 3.0;
        let zx2 = (c as f64 + 1.0) / 3.0;
        let zy1 = r as f64 / 3.0;
        let zy2 = (r as f64 + 1.0) / 3.0;
        zx1 >= center_zone_padding.left && zx2 <= 1.0 - center_zone_padding.right
            && zy1 >= center_zone_padding.top && zy2 <= 1.0 - center_zone_padding.bottom
    };
    let is_center = |rx: f64, ry: f64| -> bool {
        rx / width >= center_zone_padding.left && rx / width <= 1.0 - center_zone_padding.right
            && ry / height >= center_zone_padding.top && ry / height <= 1.0 - center_zone_padding.bottom
    };
    let zone_center_px = |r: usize, c: usize| -> (f64, f64) {
        ((c as f64 + 0.5) / 3.0 * width, (r as f64 + 0.5) / 3.0 * height)
    };

    // Sort zone keys: periphery zones first (they get first chance to overflow)
    let mut zone_list: Vec<(usize, usize)> = zone_buckets.keys().copied().collect();
    zone_list.sort_by(|&a, &b| {
        let a_c = is_center_zone(a.0, a.1);
        let b_c = is_center_zone(b.0, b.1);
        a_c.cmp(&b_c) // periphery (false) before center (true)
    });

    for _ in 0..100 {
        let mut moved_any = false;
        for zone in &zone_list {
            let cap = zone_cap(zone.0, zone.1);
            let bucket_len = zone_buckets.get(zone).map_or(0, |b| b.len());
            if bucket_len <= cap {
                continue;
            }
            let mut excess = bucket_len - cap;
            // Sort neighbors: center zones first (preferred overflow targets)
            let mut nbrs = neighbors(zone.0, zone.1);
            nbrs.sort_by(|&a, &b| {
                let a_c = is_center_zone(a.0, a.1);
                let b_c = is_center_zone(b.0, b.1);
                b_c.cmp(&a_c) // center (true) before periphery (false)
            });
            for nbr in nbrs {
                let nbr_cap = zone_cap(nbr.0, nbr.1);
                let nbr_len = zone_buckets.get(&nbr).map_or(0, |b| b.len());
                let space = nbr_cap.saturating_sub(nbr_len);
                if space == 0 {
                    continue;
                }
                let (ncx, ncy) = zone_center_px(nbr.0, nbr.1);

                // Sort by distance to neighbor center
                if let Some(bucket) = zone_buckets.get_mut(zone) {
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
                    .get_mut(zone)
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

    // Global pass: move excess from any zone to ANY zone with space
    for _ in 0..10 {
        let mut moved_any = false;
        for zone in &zone_list {
            let cap = zone_cap(zone.0, zone.1);
            let bucket_len = zone_buckets.get(zone).map_or(0, |b| b.len());
            if bucket_len <= cap { continue; }
            let mut excess = bucket_len - cap;

            let mut targets: Vec<((usize, usize), usize)> = zone_buckets
                .iter()
                .filter(|(&k, _)| k != *zone)
                .map(|(&k, b)| (k, zone_cap(k.0, k.1).saturating_sub(b.len())))
                .filter(|(_, space)| *space > 0)
                .collect();
            targets.sort_by_key(|(_, space)| std::cmp::Reverse(*space));
            for (target, space) in targets {
                let to_move = space.min(excess);
                let moved: Vec<usize> = zone_buckets
                    .get_mut(zone)
                    .map(|b| b.drain(..to_move).collect())
                    .unwrap_or_default();
                zone_buckets.entry(target).or_default().extend(moved);
                excess -= to_move;
                moved_any = true;
                if excess == 0 { break; }
            }
        }
        if !moved_any { break; }
    }

    log::debug!("Zone distribution after overflow:");
    for (&(r, c), bucket) in &zone_buckets {
        let cap = zone_cap(r, c);
        let n = bucket.len();
        let n_keys = first_key_zones[r][c].len();
        let center = if is_center_zone(r, c) { "center" } else { "periphery" };
        log::debug!("  zone ({},{}) {}: {} children, cap={} ({} keys){}",
            r, c, center, n, cap, n_keys,
            if n > cap { format!(" → {} will need 3-char!", n - cap) } else { String::new() }
        );
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
    for (&(row, col), zone_children) in &mut zone_buckets {
        let zone_keys: Vec<char> = first_key_zones[row][col].chars().collect();
        let n = zone_children.len();

        if n <= zone_keys.len() {
            // Single-char hints — sort periphery first so they get priority
            zone_children.sort_by(|&a, &b| {
                let (ax, ay) = children[a].relative_position;
                let (bx, by) = children[b].relative_position;
                let a_c = is_center(ax, ay);
                let b_c = is_center(bx, by);
                a_c.cmp(&b_c)
            });
            for (child_idx, &key) in zone_children.iter().zip(zone_keys.iter()) {
                hints.insert(key.to_string(), *child_idx);
            }
        } else {
            // Multi-char: sort periphery first so they get shorter labels
            zone_children.sort_by(|&a, &b| {
                let (ax, ay) = children[a].relative_position;
                let (bx, by) = children[b].relative_position;
                let a_c = is_center(ax, ay);
                let b_c = is_center(bx, by);
                a_c.cmp(&b_c)
            });

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
