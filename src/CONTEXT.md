# Source layout

```
src/
├── lib.rs               Public crate root — exposes all modules for integration tests
├── main.rs              Entry point, CLI, backend orchestration, action dispatch
├── child.rs             Child struct + ChildKind enum
├── config.rs            Config structs, serde JSON merge loading
├── filter.rs            filter_tiny + cull_overlaps (shared by main.rs and tests)
├── hints.rs             Spatial zone-based hint label generation
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

tests/
└── screenshot_benchmarks.rs  Run screenshots through detection, check hint counts

test-assets/screenshots/      PNG fixtures for the benchmark (filename-encoded thresholds)
```
