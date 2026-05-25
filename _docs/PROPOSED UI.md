# Proposed UI

Note: **This is not a final spec.** I am not sure if it's what I want! But it's a fully-generated spec from one session with Claude Design that I don't *dis*like and is a coherent direction. Saving for **reference, not concrete implementation.** Canonical aesthetics are still being deferred and have more considerations to factor in.

Crucially, the mocks were in 2D--I think I need to wait until my usage budget resets, then have variants redrawn in 3D, because too much changes with the game board rendered accurately.

Also would like to keep a rainbow theme as opposed to any monochrome accent color--return to this.

Things that are probably being kept: I'm drawn to terminal-y/hacker aesthetics, but the shuffleboard needs a bit more texture.

---

## akius — aesthetic spec

A handoff doc for a coding agent (or future-me). Reference for the akius
HUD, in-game effects, and any new UI surfaces.

---

## Identity

**akius** is a 3D-shuffleboard merge game. The board view is the hero — a
tactile, physical 3D scene. The 2D UI on top should feel like a **diagnostic
terminal reading out the state of that scene**, not like a typical game HUD.

The aesthetic triangulates between two poles I committed to:

- **"A · Phosphor Terminal"** — clean, restrained, hacker CRT. Amber phosphor
  on near-black, ASCII chrome, scanlines, log lines.
- **"E · Sysadmin Console"** — data-rich tmux-style tiled panels with
  cursor coordinates, htop bargraphs, live log streams.

**Target: closer to A, with the *spirit* of E.** Pull a few specific
data-richness moves from E (cursor coordinates, occasional inspect-element
tags) but don't tile the whole screen — the 3D board needs room to breathe
and stay the star.

### One-line vibe

> An old serial terminal patched into a real shuffleboard table.

---

## Palette

### Chrome (the UI itself)

| Token | Hex | Use |
|---|---|---|
| `--bg` | `#0a0a08` | Near-black backdrop. Behind everything. |
| `--deep` | `#04040a` | Recessed inset panels, CRT screen interior. |
| `--phosphor` | `#ffb733` | Amber phosphor. **Primary chrome color.** |
| `--phosphor-bright` | `#fff0c8` | Highlights, hot text, active state. |
| `--phosphor-dim` | `#9a6f1a` | Secondary labels, dim chrome. |
| `--ok` | `#7fff7a` | Success / "ON" / order fulfilled. Used sparingly. |
| `--danger` | `#ff3a4b` | Foul / overflow / red alert. |
| `--cyan` | `#4dd6ff` | Reserved for "inspect" / cursor coordinate readouts. |

Rule: the chrome stays monochrome amber 90% of the time. Color (cyan / green
/ red) only appears for **status**, never decoration.

### Orbs (the game pieces)

Tiers 1–10 follow a fixed rainbow, cool→warm, small→large. **Never invent
new orb colors.** When the orb sits on a dark amber UI, give it a strong
outer glow to match the phosphor language.

| Tier | Hex | Name | Diameter |
|---|---|---|---|
| 1 | `#B85FE3` | Amethyst | 22px |
| 2 | `#6B7AE8` | Indigo | 30 |
| 3 | `#4FA3F7` | Azure | 40 |
| 4 | `#4DD4D4` | Cyan | 52 |
| 5 | `#4CC57C` | Emerald | 66 |
| 6 | `#A8D84A` | Lime | 82 |
| 7 | `#F5D442` | Citrine | 100 |
| 8 | `#F5A742` | Amber | 118 |
| 9 | `#F26B5C` | Coral | 138 |
| 10 | `#E84B7C` | Ruby | 158 |

(Tier names are optional flavor — feel free to drop them if not used. The
game itself can stay number-only.)

---

## Typography

**One font: JetBrains Mono.** Weights 400, 500, 700, 800.

- `400` — body labels, log lines, captions
- `500` — buttons, default chip labels
- `700` — emphasized labels, "ORDER", "DELIVERED"
- `800` — the score, big numerical readouts

### Type rhythm

| Element | Size | Weight | Letter-spacing |
|---|---|---|---|
| Score (hero number) | 38–42px | 800 | -0.5 |
| Order / Next title (heading) | 18px | 700 | 2 |
| Section label (all-caps tiny) | 9–10px | 500–700 | 2–3 |
| Body / log line | 11–12px | 400–500 | 0–1 |
| Cursor info-tag | 10px | 500 | 1 |
| Tiny axis ticks | 8px | 400 | 1 |

**Numbers use `font-variant-numeric: tabular-nums`** everywhere — score,
times, coordinates. They must align in columns.

**Always-caps:** section labels, status badges (`[ON]`, `[OK]`, `RUN`).
**Mixed-case:** prose and log lines (`merge T6+T6→T7 +3200`).

---

## Chrome — ASCII / box-drawing vocabulary

UI panels are drawn with **1.5px solid amber borders + corner registration
marks**, and use Unicode box-drawing characters for inline emphasis:

```
┌─ score ──────────────┐    ┌─ next ─────┐
│ $ cat score          │    │ queue[0]   │
│ 094 900              │    │ T3 azure   │
│ ▲ +12 400  hi 128k   │    └────────────┘
└──────────────────────┘

╔══ ORDER 0x2A ══╗
║  DELIVER ● ×1  ║
║  T9 // 25 000  ║
╚════════════════╝
```

The labels above each panel sit **on top of the border**, hanging from the
top-left like a section title in old DOS UIs.

**Cursor coordinate readouts** (the move borrowed from sysadmin): the board
view has a tiny `cursor (240, 355)` readout that updates as the player
sweeps the drop position. The same readout repeats inside the
inspect-element tags below.

---

## Layout — the actual HUD

Desktop / wide:

```
┌─── akius@parlor:~$ ──────────────────────────────────────────────┐
│ logo + path                                                       │
│                                                                   │
│ [SCORE]                  [ORDER 0x2A]                  [NEXT]    │
│  094 900                  DELIVER ● ×1                  T3 azure │
│  +12 400                  T9 // 25 000                  queue[1..3] │
│                                                                   │
│                  ┌────────────────────┐                          │
│                  │  /dev/board0        │                          │
│                  │  ┌──┐               │      STDOUT             │
│                  │  │3D scene goes    │      15:42:01 merge ... │
│                  │  │ here. 2D HUD    │      15:42:09 merge ... │
│                  │  │ stays out of    │      15:42:14 ORDER     │
│                  │  │ its way.        │      ▮                  │
│                  │  └──┘               │                          │
│                  │  └─ y=500 overflow─ │                          │
│                  └────────────────────┘                          │
│  [CHAIN ×3]   [tier ladder strip]      [colorblind = [on]]      │
└──────────────────────────────────────────────────────────────────┘
```

**Mobile portrait:** stack vertically. Score at top, board taking the
middle 60–70vh, next/queue + chain along the bottom. Hide the log stream;
keep only score, order, next, ladder, and the colorblind toggle.

### Spacing

- Outer margin from viewport: **36px** desktop, **16px** mobile
- Panel padding: **10–14px** vertical, **16–18px** horizontal
- Panel corner-tick offset: **-4px** from the panel edge
- Gap between adjacent panels: **24px**

### Borders

- All panel borders: `1.5px solid var(--phosphor)`
- Panel glow: `box-shadow: 0 0 8px var(--phosphor)55, inset 0 0 22px var(--phosphor)22`
- Corner registration ticks: 6px L-shapes in `--phosphor`
- The score, order, and next chips share identical border treatment
- **Never use rounded corners.** Every panel is a sharp rectangle.

---

## Specific HUD elements

### Score chip
- Top-left.
- Label: `SCORE.LOG` above the border.
- Prompt line: `$ cat score` in `--phosphor-dim`.
- Number: 38–42px, weight 800, `--phosphor-bright` with strong text-shadow glow.
- Delta line: `▲ +12,400 | best 128,440` in `--phosphor`.

### Order chip
- Top-center.
- Label: `╔══ ORDER 0x{hex} ══╗` (order ID rendered in hex for flavor).
- Body: `DELIVER {orb glyph} ×{count}` — the orb is the actual rendered
  orb, not a colored square.
- Footer: `T{n} {NAME} // {payout} pts`.

### Next + queue chip
- Top-right.
- Label: `NEXT`.
- Primary: the orb + `T{n}` in big numerals.
- Queue: a row of 3 smaller orbs labeled `queue[1..3]`.

### Tier ladder
- Below the board, full board width.
- 10 small orbs in a row.
- Reached tiers full saturation; unreached are ghosted (opacity 0.32).
- The current target tier is wrapped in an outline ring + a small `◆ TARGET` label.

### Chain / combo
- Bottom-left.
- `×3` in 28–32px, weight 800.
- Below: a 5-segment progress strip `▰▰▰▱▱` showing chain timer.

### Colorblind toggle
- Bottom-right.
- `colorblind = [on]` — `on` rendered in `--ok` green.
- The only place green appears in the chrome by default.

### Log stream (optional, desktop only)
- Right side of the board, narrow column.
- Header: `STDOUT` in `--phosphor-dim`.
- Lines, newest at bottom: `{HH:MM:SS} {event} {delta}`.
- Successful order fulfillment is highlighted in `--ok`.
- A `▮` cursor blinks at the bottom of the stream.

### Cursor readout (borrowed from sysadmin)
- A small `cursor ({x}, {y})` overlay near the top of the board area.
- Updates as the drop position moves.
- Color: `--cyan` to differentiate from chrome.

---

## In-game effects

### Drop / aim
There is **no aim line** — the ball always drops straight down from the
launcher. The only thing the player chooses is the X position.

Show this as a **vertical phosphor-dotted line** from the launcher straight
up to where the next orb will enter the board, with a small dotted circle
at the top showing the projected entry point. Update its X live as the
player moves.

### Order fulfillment
When an orb of the target tier is reached, it leaves the board with this
sequence (~600–800ms total):

1. **Crosshair brackets** (`┌ ┐ └ ┘`) snap onto the orb corners, in
   `--phosphor`, with a soft pulse. (~150ms)
2. **Scan beam** sweeps top→bottom across the orb interior — a thin
   horizontal `--phosphor-bright` band with strong glow. (~300ms)
3. **Center text** appears: `[ ACQUIRED ]` in `--phosphor-bright` with
   heavy text-shadow glow. (~150ms)
4. **Log line** prints below the orb: `> deliver(T9) // +25000` in
   `--phosphor`.
5. Orb fades out + dissolves into upward-rising voxel particles
   (~200ms), then the score increments.

The same event simultaneously prints into the STDOUT log column.

### Merge burst
When two orbs combine, a quick phosphor halo (~250ms) — single ring
expanding from center and fading. No confetti, no theatrics. The new
orb's color does the celebrating.

### Danger / overflow line
Dashed 2px `--danger` line across the board at y=500-ish. Label:
`[!] OVERFLOW THRESHOLD` in `--danger` to the left. When an orb crosses
it, the line **pulses** (opacity 0.6 → 1.0 → 0.6, ~400ms loop).

### Game over
Full-screen blackout with: `FATAL: board.overflow` in `--danger`,
followed by `final.score = 094 900` in `--phosphor-bright`, followed
by `[ press space to restart ]` in `--phosphor-dim`.

---

## Motion

- **Easings:** `cubic-bezier(.2,.7,.3,1)` for entries, `cubic-bezier(.4,0,.6,1)` for exits.
- **Durations:** UI feedback 120ms; status changes 220ms; fulfillment sequence 600–800ms.
- **CRT flicker:** a very subtle 4–6 second cycle on the entire chrome layer
  (`opacity: 1 → 0.96 → 1`). Don't overdo it. **Disable when**
  `prefers-reduced-motion: reduce`.
- **Scanlines:** repeating 1px-on / 2px-off horizontal stripes at ~6%
  opacity over the whole chrome layer. **Behind the board's 3D scene, not
  over it.**

---

## Rules of taste

**Do:**
- Keep everything monospace.
- Use ASCII glyphs (`▮ ▰ ▱ ▲ ▼ ◆ ● ★ ┌ ┐ └ ┘ ╔ ╗ ╚ ╝ ║ ═`) for ornament.
- Show the player's actions as if a process is logging them.
- Let the orb colors be the only saturated color most of the time.
- Use tabular numerals everywhere.

**Don't:**
- Rounded corners.
- Decorative gradients on chrome.
- Drop shadows other than the phosphor glow.
- Icons that aren't ASCII / box-drawing glyphs.
- More than one non-phosphor color in the chrome at once.
- Stack so many tmux-style panels that the 3D board feels cramped.
- Add tier names or order hex IDs visibly if they don't help the player.

---

## What we're trading off vs. the sysadmin version

The sysadmin direction proved that a console UI can carry an absurd amount
of data — recent merges, render stats, keymap, settings, run stats — and
still look beautiful. The reason we're pulling back from full tmux is:

1. The **3D board is the hero**. A wall of side panels visually competes with it.
2. Most of that data isn't actionable while playing. It's beautiful but it's noise.
3. Mobile portrait can't carry tiled panels at all.

So we **keep** the moves that *improve* play:
- Live coordinate readout (helps targeting)
- Inspect-element style fulfillment tag (gives weight to scoring moments)
- Log stream column on the right (optional, desktop only, can be toggled
  off — it's an "extras" panel)

And we **drop** the moves that are pure data:
- Render stats, run stats, keymap panels (move to a `?` overlay)
- Settings panel (move to a settings screen)
- Multi-section status bars at top/bottom

If a future version needs to feel more "operator-y," reintroduce panels
one at a time and validate they're helping.

---

## Files of record (from the design canvas)

- `akius designs.html` — the canvas with all directions and remixes.
- `direction-mono.jsx` — section-1 brutalist mono refined (the closest
  starting point if you ignore the phosphor recolor).
- `remix-phosphor.jsx` — **the primary reference for this spec.**
- `remix-sysadmin.jsx` — for the cursor readout, inspect tag, and log
  stream patterns we kept.
- `orb-helpers.jsx` — shared 10-tier palette, the `Orb` component (use
  `variant="neon"` for phosphor glow), and the `TierLadder`.
