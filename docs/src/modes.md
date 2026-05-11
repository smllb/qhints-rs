# Modes

## Normal mode

Type a hint → click at element center.

| Modifier | Effect |
|----------|--------|
| (none) | Click left button |
| Hold Ctrl on last key | Hover (mousemove, no click) |

## Double-click mode

Press **Alt** (configurable `double_click_key`) to toggle. The next hint you type
double-clicks that element. Mode auto-resets after one use.

Visual feedback: thicker label border when active.

## Text selection mode

Press **`/`** (configurable `text_select_key`) to toggle. Select text between two
elements.

### Normal text selection

1. Press `/` to enter text selection mode (blue borders appear on all elements)
2. Type a hint to place the **start marker** (red vertical line)
3. Type a second hint → selection is executed immediately:
   - Text words select from word-edge to word-edge
   - UI elements select from center to center

### Advanced text selection

1. Press `/` to enter text selection mode
2. Type first hint → start marker placed
3. Press `/` or **Ctrl** again to enter **advanced mode** (hints hide, only markers shown)
4. Type second hint → **end marker** placed (orange vertical line)
5. Fine-tune with controls:

| Key | Effect |
|-----|--------|
| Arrow keys | Nudge active marker position |
| Shift + arrow | Larger nudge |
| Tab | Switch between start/end marker |
| Enter | Confirm and execute selection |

## Drag mode

Press **Shift** (configurable `drag_key`) to toggle. Drag an element to a new position.

### Basic drag

1. Press Shift to enter drag mode (green borders appear)
2. Type first hint → **source** element selected
3. Type second hint → **destination** picked, drag executes immediately
   - Mouse moves from source center to destination center
   - Smooth interpolation at 8px steps

### Fullscreen drag

After placing the source (step 2), press **Shift** again to re-scan the entire
monitor. This lets you pick a destination anywhere on screen, not just in the
current window.

### Advanced drag

1. Press Shift, place source marker
2. Press **Ctrl** to enter advanced drag mode
3. Type second hint → destination marker placed
4. Arrow keys to nudge, Tab to switch, Enter to confirm

## Hunt mode

With `hunt: true` in config, the overlay re-appears after every action so you can
perform multiple operations in sequence.

- Hold **Ctrl** during hunt to signal "this is the last one" — next action exits
- Auto-dismisses after 10 seconds of inactivity

## Mode combos

| Sequence | Result |
|----------|--------|
| Alt → hint | Double-click |
| `/` → hint → hint | Select text between two elements |
| `/` → hint → `/` → hint → (arrows) → Enter | Advanced text select |
| Shift → hint → hint | Drag source to destination |
| Shift → hint → Shift → (pick anywhere) → hint | Fullscreen drag |
