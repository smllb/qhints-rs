# src/mouse.rs

**Note:** Unused. All mouse dispatch is inlined in `main.rs`.

```rust
pub fn click(
    x: i32,
    y: i32,
    button: u32,
    repeat: u32,
) -> Result<(), Box<dyn std::error::Error>>
```

Shells out to `xdotool mousemove X Y click BUTTON`, repeated `repeat` times.
