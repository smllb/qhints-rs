# Quantum Hints

![screenshot](https://i.redd.it/e4ilwsc0pk0h1.gif)

Keyboard-driven UI navigation tool for Linux — Rust rewrite of [qhints](https://github.com/smllb/qhints) (a fork of [hints](https://github.com/AlfredoSequeida/hints) by Alfredo Sequeida). Shows labelled overlays on screen elements, letting you click/hover them via keyboard.

**Documentation:** https://smllb.github.io/qhints-rs/ — Demonstration: https://youtu.be/BWC7h5dmkI4

---

## Installation

### Option A — Download the prebuilt binary (easiest)

Grab the latest release from [GitHub Releases](https://github.com/smllb/qhints-rs/releases):

```sh
curl -L -o qhints-rs https://github.com/smllb/qhints-rs/releases/latest/download/qhints-rs
chmod +x qhints-rs
sudo mv qhints-rs /usr/local/bin/
```

### Option B — Build from source

#### Dependencies

- **X11 session** (Wayland is not yet supported — see [Wayland](#wayland))
- **AT-SPI D-Bus service** (`at-spi-dbus-bus.service`) for the accessibility backend
- **Rust + Cargo 1.87+** — use [rustup](https://rustup.rs); distro packages are often too old (e.g. Debian's rustc 1.85 will not work)
- **A compositor is recommended** (tested with picom on i3)
- For OCR (optional): `clang libclang-dev`

##### Debian/Ubuntu/Mint

```sh
sudo apt install xdotool libgtk-3-dev librsvg2-dev
```

For OCR: `sudo apt install clang libclang-dev`

##### Rust toolchain

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

#### Build

```sh
cargo build --release
```

For OCR support (optional):

```sh
cargo build --release --features ocr
```

**Note:** The OCR backend downloads ~35MB of models (`text-detection.rten`, `text-recognition.rten`) from AWS S3 on first run to `~/.cache/qhints/ocrs/` ([code](src/backend/ocrs.rs)).

The binary is at `target/release/qhints-rs`.

---

## Usage

Run directly:

```sh
qhints-rs
```

Or via the wrapper script (logs to syslog):

```sh
./scripts/run-qhints.sh
```

### i3 keybinding example

```conf
bindsym ctrl+shift+p exec --no-startup-id /home/yogi/qhints-rs/scripts/run-qhints.sh
```

### Modes

| Key | Behavior | How to use | Advanced |
|-----|----------|------------|----------|
| Type hint keys | Click an element | Type its label | — |
| Ctrl on last key | Hover instead of click | Type the label, hold Ctrl on the last key | — |
| **Alt** (toggle) | Double-click next pick | Press Alt → type a label | — |
| **/** (toggle) | Select text | Press `/` → pick start → pick end | After picking start, press `/` or **Ctrl** → pick end → arrow keys to adjust → **Tab** to switch sides → **Enter** to confirm |
| **Shift** (toggle) | Drag something | Press Shift → pick source → pick destination | After picking source, press **Ctrl** → pick target → arrow keys to adjust → **Tab** to switch sides → **Enter** to confirm. Or press **Shift** again to see all monitors and pick a target anywhere. |
| **Escape** | Dismiss overlay | Press Escape | — |

### Options

| Flag | Description |
|------|-------------|
| `-m`, `--mode` | `hint` (default) or `scroll` |
| `-v` | Verbosity (`-v` = debug, `-vv` = trace) |

---

## Configuration

Config file: `~/.config/qhints/config.json` (or `$XDG_CONFIG_HOME/qhints/config.json`). All fields are optional — defaults are used for missing keys.

See the [Configuration docs](https://smllb.github.io/qhints-rs/configuration.html) for the full list of fields, defaults, and per-application rules.

---

## Backends

1. **AT-SPI** (primary) — walks the accessibility tree via D-Bus. Fast, async, respects application roles and states. Needs `at-spi-dbus-bus.service` running.
2. **ocrs** (optional, feature-gated) — OCR text detection. Produces word-level hints + BFS gap-filling for icons.
3. **Imageproc** (fallback) — Canny edge detection + BFS connected components + text line projection.

All configured backends run in order and their results are merged. Overlap culling prefers `Text` children over `Element` when overlap exceeds 80%.

---

## Wayland

qhints-rs currently depends on:

- **x11rb** — window geometry, focus tracking, input emulation
- **xdotool** — mouse click simulation
- **GTK3 overlay** — may work under XWayland but is untested

To support Wayland natively, the following would need to be added:

- A `window_system/wayland.rs` backend using a Wayland client library for window info
- `ext-image-capture-src` or similar protocol for screenshot capture
- `libei` / `ydotool` for input emulation
- Testing across compositors (GNOME/KDE/Sway/Hyprland)

## Status

**Tested on X11 only.** Wayland is not yet supported (see [Wayland](#wayland)).

---

If this project helps you, consider sponsoring: https://github.com/sponsors/smllb

Contributions welcome.