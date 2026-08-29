use qhints_rs::backend::imageproc;
use qhints_rs::child::ChildKind;
use qhints_rs::config::ApplicationRule;

fn main() {
    let rule = ApplicationRule::default();
    let mut files: Vec<_> = std::fs::read_dir("test-assets/screenshots")
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|s| s == "png").unwrap_or(false))
        .collect();
    files.sort();
    for f in files {
        let img = image::open(&f).unwrap();
        let d = imageproc::detect_children_debug(&img, &rule, 0.0, 0.0).unwrap();
        let (t, e) = d.children.iter().fold((0, 0), |(t, e), c| match c.kind {
            ChildKind::Text => (t + 1, e),
            ChildKind::Element => (t, e + 1),
        });
        // height histogram of pieces (area-weighted)
        use std::collections::HashMap;
        let mut bins: HashMap<u32, f64> = HashMap::new();
        for p in &d.pieces {
            let h = p.height.round() as u32;
            if (6..=60).contains(&h) {
                *bins.entry(h).or_insert(0.0) += p.width * p.height;
            }
        }
        let mut v: Vec<_> = bins.into_iter().collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top: Vec<String> = v.iter().take(6).map(|(h, c)| format!("{}x{:.0}", h, c)).collect();
        let (t, e) = d.children.iter().fold((0, 0), |(t, e), c| match c.kind {
            ChildKind::Text => (t + 1, e),
            ChildKind::Element => (t, e + 1),
        });
        println!(
            "{}: text_h={:.0} final={}(T={},E={}) | aw_hist={}",
            f.file_name().unwrap().to_string_lossy(),
            d.text_h,
            d.children.len(),
            t,
            e,
            top.join(", ")
        );
        // dump small pieces for the second screenshot
        if f.file_name().unwrap().to_string_lossy().contains("1450_1_1") {
            println!("  small (h<=8) pieces (x,y,w,h):");
            for p in d.pieces.iter().filter(|p| p.height <= 8.0).take(40) {
                println!(
                    "    ({:.0},{:.0}) {}x{}",
                    p.relative_position.0, p.relative_position.1, p.width, p.height
                );
            }
        }
    }
}