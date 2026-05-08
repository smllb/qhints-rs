/// Distinguishes text content (words) from other UI elements (buttons, icons, etc.).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChildKind {
    Element,
    Text,
}

/// Child element representing an accessible UI element on screen.
#[derive(Debug, Clone)]
pub struct Child {
    /// Position relative to the focused window's top-left corner.
    pub relative_position: (f64, f64),
    /// Absolute position on screen.
    pub absolute_position: (f64, f64),
    /// Element width in pixels.
    pub width: f64,
    /// Element height in pixels.
    pub height: f64,
    /// Whether this child is text content or a regular UI element.
    pub kind: ChildKind,
}
