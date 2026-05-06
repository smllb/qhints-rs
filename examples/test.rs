use std::process::Command;

fn main() {
    println!("Testing xdotool from Rust...");
    let output = Command::new("xdotool")
        .args(["mousemove", "2904", "986", "click", "1"])
        .output()
        .expect("Failed to run xdotool");
    
    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    println!("status: {}", output.status);
}