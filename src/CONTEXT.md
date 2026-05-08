# Source layout

```
src/
├── main.rs              Entry point, CLI, backend orchestration, action dispatch
├── child.rs             Child struct + ChildKind enum
├── config.rs            Config structs, JSON merge loading
├── hints.rs             Spatial zone-based hint label generation
├── mouse.rs             xdotool helper (unused, dispatch in main.rs)
├── backend/
│   ├── atspi.rs         AT-SPI D-Bus accessibility tree walker
│   ├── imageproc.rs     Image-based detection (Canny + BFS + text lines)
│   └── ocrs.rs          OCR + BFS gap-filling (feature-gated)
├── overlay/
│   ├── mod.rs           GTK popup, keyboard grab, state machine
│   └── drawing.rs       Cairo rendering, overlap culling, spotlight
└── window_system/
    ├── mod.rs           WindowInfo + WindowSystem trait
    └── x11.rs           X11 implementation
```
