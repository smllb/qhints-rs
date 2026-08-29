//! Live detection pipeline debug viewer (GTK).
//!
//! Run with: `cargo run --release --example preview`
fn main() {
    gtk::init().expect("Failed to initialize GTK");
    qhints_rs::debug_viewer::run();
}