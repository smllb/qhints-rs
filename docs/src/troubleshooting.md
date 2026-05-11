# Troubleshooting

## Overlay doesn't appear

Check you're on X11:

```sh
echo $XDG_SESSION_TYPE  # should say "x11"
```

Verify AT-SPI is running:

```sh
systemctl --user status at-spi-dbus-bus.service
```

Run from a terminal with `-v` to see logs:

```sh
./qhints-rs -v
```

## Keyboard input not reaching overlay

The keyboard grab can fail with some window managers. Click on the overlay
to dismiss it (acts as safety net). Use `-vv` for trace-level logs.

## No elements detected

Some applications (VS Code, browsers) don't expose accessibility info.
Try adding `imageproc` to your backends:

```json
{ "backends": ["imageproc"] }
```

If that works but AT-SPI doesn't, your app may need `--force-renderer-accessibility`
or similar flags.

## "qhints already running"

The lock file at `/tmp/qhints.lock` prevents concurrent instances. If qhints
crashed, remove it:

```sh
rm /tmp/qhints.lock
```

## Overlay on wrong monitor

Use `overlay_x_offset` and `overlay_y_offset` to shift. With i3 + multi-monitor,
the overlay may need adjustments:

```json
{
  "overlay_x_offset": 1920,
  "overlay_y_offset": 0
}
```

## Can't find config file

Config path: `~/.config/qhints/config.json`. Create it if missing — all
fields are optional.

## Hints too large / too small

Adjust `hint_font_size` and `hint_height`:

```json
{
  "hints": {
    "hint_font_size": 12,
    "hint_height": 18
  }
}
```

## Selection/drag precision issues

Decrease nudge step values for finer control:

```json
{
  "hints": {
    "text_select_nudge_step_x": 0.01,
    "text_select_nudge_step_y": 0.01
  }
}
```

## Build fails

Ensure system deps are installed:

```sh
sudo apt install xdotool libgtk-3-dev librsvg2-dev clang libclang-dev
```

For the OCR feature (enabled by default), you need `clang` + `libclang-dev`.
Disable OCR if you don't need it:

```sh
cargo build --release --no-default-features
```

## GPU/performance

The overlay uses software rendering via Cairo. On slower systems, increase
`marker_pulse_interval_ms` to reduce redraw frequency:

```json
{
  "hints": {
    "marker_pulse_interval_ms": 50
  }
}
```

## Wayland

qhints-rs depends on:
- `x11rb` — window geometry and focus tracking
- `xdotool` — mouse input simulation
- GTK3 overlay (may work under XWayland, untested)

Wayland support would require:
- A Wayland client library for window info
- `ext-image-capture-src` for screenshots
- `libei` / `ydotool` for input

Contributions welcome.
