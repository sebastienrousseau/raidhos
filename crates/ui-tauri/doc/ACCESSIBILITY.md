<!-- SPDX-License-Identifier: GPL-3.0-only -->

# `raidhos-ui` Accessibility

The UI targets **WCAG 2.2 AA** conformance. This document maps
each shipping control to the WCAG success criterion it
addresses and to the source file that implements it. Use it as
a verification checklist when changing the frontend.

For broader UX context, see
[`docs/USER_GUIDE.md`](../../../docs/USER_GUIDE.md).

---

## Conformance map

### Perceivable

| WCAG SC | Control | Source |
|---|---|---|
| 1.1.1 Non-text Content | Every image has descriptive `alt` text; the logo is decorative and uses `alt=""` | [`frontend/index.html`](../frontend/index.html) |
| 1.3.1 Info and Relationships | `<main>`, `<nav>`, `<section>`, `role="dialog"`, `role="toolbar"`, `role="log"` | same |
| 1.3.2 Meaningful Sequence | DOM order matches visual order; no `tabindex` shuffling | same |
| 1.4.3 Contrast | Light theme tested at 4.5:1; high-contrast variant 7:1+ | [`frontend/styles.css`](../frontend/styles.css) |
| 1.4.4 Resize Text | All sizing in `rem` / `em`; viewport meta allows pinch-to-zoom | same |
| 1.4.10 Reflow | Layout collapses to a single column < 480px without horizontal scroll | same |
| 1.4.11 Non-text Contrast | Focus ring on every focusable; 3:1+ against background | same |
| 1.4.12 Text Spacing | `line-height: 1.5` baseline | same |
| 1.4.13 Content on Hover | No hover-only content; popovers dismissible with Escape | same |

### Operable

| WCAG SC | Control | Source |
|---|---|---|
| 2.1.1 Keyboard | Every interactive element reachable + activatable with `Tab` / `Enter` / `Space` | [`frontend/app.js`](../frontend/app.js) |
| 2.1.2 No Keyboard Trap | Modals trap focus *within themselves* but Escape always releases | same |
| 2.1.4 Character Key Shortcuts | None (no single-letter shortcuts) | — |
| 2.4.1 Bypass Blocks | Skip-to-main link at the top of every page | [`frontend/index.html`](../frontend/index.html) |
| 2.4.3 Focus Order | Logical: Discover → Pick → Confirm → Progress | same |
| 2.4.4 Link Purpose | Every link text describes the destination in context | same |
| 2.4.6 Headings and Labels | Every form input has an associated `<label for="…">` | same |
| 2.4.7 Focus Visible | `:focus-visible` ring on every focusable | [`frontend/styles.css`](../frontend/styles.css) |
| 2.5.1 Pointer Gestures | No multi-finger / path-based gestures | — |
| 2.5.7 Dragging Movements | Drag-drop ISOs has a click-to-pick fallback | [`frontend/app.js`](../frontend/app.js) |
| 2.5.8 Target Size | All interactive targets ≥ 24×24 CSS pixels | [`frontend/styles.css`](../frontend/styles.css) |

### Understandable

| WCAG SC | Control | Source |
|---|---|---|
| 3.1.1 Language of Page | `<html lang="en">` set | [`frontend/index.html`](../frontend/index.html) |
| 3.2.1 On Focus | Focus never triggers state changes | [`frontend/app.js`](../frontend/app.js) |
| 3.2.2 On Input | Form-state changes are explicit; no `change="form.submit()"` | same |
| 3.3.1 Error Identification | Validation errors appear inline below the offending field | same |
| 3.3.2 Labels or Instructions | Every input has a visible label and (where applicable) help text | [`frontend/index.html`](../frontend/index.html) |
| 3.3.3 Error Suggestion | Each error suggests a fix when one is known | [`frontend/app.js`](../frontend/app.js) |
| 3.3.4 Error Prevention | Destructive flash requires typing `ERASE`; no double-tap-to-destroy | same |

### Robust

| WCAG SC | Control | Source |
|---|---|---|
| 4.1.2 Name, Role, Value | Every custom widget has appropriate ARIA + keyboard semantics | [`frontend/index.html`](../frontend/index.html) |
| 4.1.3 Status Messages | Progress log is `role="log"` with `aria-live="polite"` | same |

---

## Honoured media queries

```css
@media (prefers-color-scheme: light)   { /* light theme */ }
@media (prefers-color-scheme: dark)    { /* dark theme */ }
@media (prefers-contrast: more)        { /* high-contrast */ }
@media (prefers-reduced-motion: reduce){ /* no transitions / no auto-scroll */ }
```

The dark theme is the default to keep parity with system /
GNOME / macOS / Windows dark mode users (who are the majority
of installer users).

---

## Testing

```bash
# axe-core via Playwright; CI gates on zero violations.
make ui-accessibility
```

Manual checks before each release:

- Tab through every interactive element in
  Chrome / Firefox / Safari.
- VoiceOver (macOS) + Orca (Linux) + NVDA (Windows) read each
  pane title correctly.
- High-contrast mode renders without overlapping or clipped
  elements.
- Reduced-motion mode does not break the progress log.
- Window resize from 320px → 1920px preserves layout.

---

## Known gaps

- **`role="alert"` on errors** — currently polite, not assertive.
  Tracking in v0.0.2.
- **Accessible name on the device-path input** — currently only
  the visible label; v0.0.2 adds an `aria-describedby` link to
  the safety-warning paragraph.
- **High-contrast tested on dark only** — light high-contrast is
  WCAG-passing but visually unverified on Linux.

---

## See also

- [`USAGE.md`](USAGE.md) — task-oriented cookbook.
- [`../README.md`](../README.md) — UI reference.
- [`../../../docs/USER_GUIDE.md`](../../../docs/USER_GUIDE.md) —
  user-facing flow walkthrough.
