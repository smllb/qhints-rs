# Overlay

GTK popup window that captures keyboard input and renders hints with cairo.

## mod.rs
- `OverlayState`: shared state (hints, typed string, modes, consumed hints)
- `MouseAction`: returned to main.rs on match (action type + coordinates + repeat)
- `show_overlay()`: creates GTK window, connects events, runs main loop
- Key event state machine: Escape → dismiss, modifier keys → mode toggles, unicode → hint match
- `select_position()`: computes mousedown/mouseup coords for text selection with padding

## drawing.rs
- `draw_hints()`: cairo rendering with text extents, overlap culling, per-character coloring
- Visual feedback: blue border in text selection (Text hints), thicker border in double-click mode
- Spotlight mode: radial gradient holes around matching hints
- Red vertical marker at text selection start position
