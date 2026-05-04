use std::process::Command;

pub fn click(
    x: i32,
    y: i32,
    button: u32,
    repeat: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let button_str = format!("{}", button);
    
    for _ in 0..repeat {
        Command::new("xdotool")
            .args([
                "mousemove", &x.to_string(), &y.to_string(),
                "click", &button_str,
            ])
            .status()?;
    }
    
    Ok(())
}