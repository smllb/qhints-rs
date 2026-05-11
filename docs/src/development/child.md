# src/child.rs

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChildKind {
    Element,
    Text,
}
```

```rust
#[derive(Debug, Clone)]
pub struct Child {
    pub relative_position: (f64, f64),
    pub absolute_position: (f64, f64),
    pub width: f64,
    pub height: f64,
    pub kind: ChildKind,
}
```
