# Installation

## System requirements

- X11 session (not Wayland)
- AT-SPI D-Bus service — usually ships with your desktop. Verify:
  ```sh
  systemctl --user status at-spi-dbus-bus.service
  ```
- Rust toolchain (install via [rustup](https://rustup.rs)):
  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- System packages:
  ```sh
  sudo apt install xdotool libgtk-3-dev librsvg2-dev
  ```
- For the OCR backend: `sudo apt install clang libclang-dev`
- A compositor recommended (tested with picom on i3)

## Build

```sh
git clone https://github.com/smllb/qhints-rs
cd qhints-rs
cargo build --release
```

Binary at `target/release/qhints-rs`.

Or install directly from git:

```sh
cargo install --git https://github.com/smllb/qhints-rs
```

## Keybinding

Bind to a shortcut in your WM config. i3 example:

```conf
bindsym ctrl+shift+p exec --no-startup-id /home/you/qhints-rs/target/release/qhints-rs
```

You can also use the wrapper script at `scripts/run-qhints.sh` (logs to syslog).
