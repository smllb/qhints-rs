# Quick Start

## 1. Build

```sh
git clone https://github.com/smllb/qhints-rs
cd qhints-rs
cargo build --release
```

## 2. Run

```sh
./target/release/qhints-rs
```

A transparent overlay appears over the active window. Every detected UI element
has a small yellow pill-shaped label on it, like **QA**, **QS**, **QD**.

## 3. Click something

Type the letters you see on the label. For example, if a terminal window shows
**QS** on the close button, press **Q** then **S**. The cursor moves to that
button and clicks.

- The **first letter** of each label is red, the rest is dark gray.
- As you type, matched letters turn green.
- If no hint matches what you typed, the input resets automatically.
- Press **Escape** to dismiss the overlay at any time.

## 4. Hover instead of click

Hold **Ctrl** while typing the last character — the cursor moves there but
doesn't click. Useful for tooltips and previews.

## 5. Try the other modes

If the overlay is up, press these keys to switch modes:

| Key | Mode | What it does |
|-----|------|-------------|
| (just type) | Normal | Click at element center |
| Ctrl + last key | Hover | Move mouse, no click |
| **Alt** | Double-click | Type one hint → double-click |
| **/** | Text select | Pick two elements → select between them |
| **Shift** | Drag | Pick source → pick destination → drag |

## What to try it on

- A file manager — close a window, hover a tooltip
- A text editor — select a word using `/` mode
- A browser — click buttons, drag a tab

## Verbose output

```sh
./target/release/qhints-rs -v      # debug logs
./target/release/qhints-rs -vv     # trace logs (very noisy)
```
