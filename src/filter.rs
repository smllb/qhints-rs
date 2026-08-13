use crate::child::{Child, ChildKind};

/// Remove children smaller than 0.25% of the window dimension (min 3px).
pub fn filter_tiny(children: Vec<Child>, w: f64, h: f64) -> Vec<Child> {
    let min_child_w = (w * 0.0025).max(3.0);
    let min_child_h = (h * 0.0025).max(3.0);
    children
        .into_iter()
        .filter(|c| c.width >= min_child_w && c.height >= min_child_h)
        .collect()
}

/// Convert a `hint_overlap_threshold` (0..100, where 0 = show all) into an
/// overlap fraction used by `cull_overlaps`. 0 maps to `f64::MAX` (keep all).
pub fn overlap_limit(threshold: f64) -> f64 {
    if threshold == 0.0 {
        f64::MAX
    } else {
        (100.0 - threshold) / 100.0
    }
}

/// Pairwise overlap culling. Returns a `kept` flag per child: when two
/// children overlap beyond `overlap_limit`, `Text` wins over `Element`,
/// otherwise the smaller child is culled.
pub fn cull_overlaps(children: &[Child], overlap_limit: f64) -> Vec<bool> {
    let child_rects: Vec<(usize, f64, f64, f64, f64)> = children
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.relative_position.0, c.relative_position.1, c.width, c.height))
        .collect();

    let mut kept = vec![true; children.len()];
    for i in 0..child_rects.len() {
        if !kept[i] {
            continue;
        }
        let (_, x1, y1, w1, h1) = child_rects[i];
        let area1 = w1 * h1;
        for j in (i + 1)..child_rects.len() {
            if !kept[j] {
                continue;
            }
            let (_, x2, y2, w2, h2) = child_rects[j];
            let ix1 = x1.max(x2);
            let iy1 = y1.max(y2);
            let ix2 = (x1 + w1).min(x2 + w2);
            let iy2 = (y1 + h1).min(y2 + h2);
            if ix1 < ix2 && iy1 < iy2 {
                let inter = (ix2 - ix1) * (iy2 - iy1);
                let area2 = w2 * h2;
                let min_area = area1.min(area2);
                if min_area > 0.0 && inter / min_area > overlap_limit {
                    // Prefer Text over Element (word hints survive over BFS noise)
                    let kind_i = children[i].kind;
                    let kind_j = children[j].kind;
                    if kind_i == ChildKind::Text && kind_j != ChildKind::Text {
                        kept[j] = false;
                        continue;
                    } else if kind_j == ChildKind::Text && kind_i != ChildKind::Text {
                        kept[i] = false;
                        break;
                    }
                    // Cull the SMALLER one
                    if area1 <= area2 {
                        kept[j] = false;
                    } else {
                        kept[i] = false;
                        break;
                    }
                }
            }
        }
    }
    kept
}
