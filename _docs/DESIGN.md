# akius game — Game Design Document

Draft 3.0

---

## Overview

akiuS is a physics-based puzzle game in which the player slides spheres across a table surface, merging same-level spheres to produce higher-tier spheres. The game is viewed from an isometric perspective. The session ends when spheres accumulate past the loss boundary near the player. Each session is endless — the goal is to survive as long as possible while fulfilling escalating orders and maximizing score.

**Target platform:** Web (WASM via itch.io) **Engine:** Bevy (3D) + Rapier3D **Language:** Rust

---

## Core Mechanic

The player is positioned at the near end of a rectangular table viewed from an isometric angle. Each turn, the player is given a sphere to slide. The player taps a position along the near edge to determine lateral placement; the sphere launches at a fixed power in the direction away from the player. Surface friction determines how far the sphere travels before coming to rest.

Spheres interact physically with each other and with table boundaries. Collisions cause repositioning — spheres can be nudged, deflected, or wedged by subsequent throws. Side boundaries and the far end are solid and bouncing; spheres decelerate against them until coming to a stop.

When two spheres of the same tier make contact, they merge into a single sphere of the next tier up, centered between them. If the resulting sphere is in contact with another same-tier sphere, it merges again immediately (chain merges). Chain reactions propagate until no further merges are possible.

---

## Loss Condition

A dashed line is rendered near the player's end of the table. If any sphere's position crosses back past this line, the session ends.

---

## Sphere Hierarchy

There are 13 tiers. Each tier corresponds to a spectral color, progressing through the visible spectrum from red to violet. Tiers are labeled numerically on the sphere surface (as a rendered texture overlay) to support colorblind accessibility. A toggle in settings enables high-contrast numerical labels as the primary visual identifier.

|Tier|Color|
|---|---|
|1|Red|
|2|Orange-Red|
|3|Orange|
|4|Amber|
|5|Yellow|
|6|Yellow-Green|
|7|Green|
|8|Teal|
|9|Cyan|
|10|Sky Blue|
|11|Blue|
|12|Indigo|
|13|Violet|

### Size Progression

Each tier's radius scales geometrically from the previous tier by a fixed multiplier (target: approximately ×1.3 per tier). This produces a level-13 sphere with roughly 10× the radius of a level-1 sphere. Exact multiplier to be tuned in playtesting.

**Table size target:** Large enough that 3–4 level-13 spheres could hypothetically coexist. This creates a tight, tense endgame.

---

## Ball Dispensing

Each turn, the player receives a sphere to throw. The dispensed tier is drawn from a weighted random distribution:

- Only tiers 1–5 are eligible for dispensing (players never receive high-tier spheres directly)
- Distribution is weighted toward the lower half of the eligible range (tiers 1–3 most common)
- Exact weights to be tuned in playtesting

A preview of the _next_ sphere in the queue is visible to the player at all times, enabling short-term planning (consistent with Suika Game conventions).

---

## Orders System

At all times, exactly one active order is displayed. The order specifies a target tier the player must produce (via merges). When a sphere of the target tier is created — whether by direct merge or chain reaction — the order is fulfilled and that sphere is removed from the board. A new order is immediately assigned.

### Order Generation

- Orders are drawn from a weighted distribution skewed toward mid-to-high tiers
- Difficulty escalates across the session: early orders are achievable at moderate tier levels, late orders require high-tier production
- Exact escalation curve and tier weights to be tuned in playtesting

### Order UI

- Displays the target tier's color and number
- No queue is shown; the next order is revealed only on completion of the current one (preserves tension)

---

## Scoring

Three scoring components combine into a session total:

**Per-merge points** Each successful merge awards points scaled to the resulting tier. Higher-tier merges award more points. Chain reactions accumulate all merge scores individually.

**Peak tier bonus** A running record of the highest tier produced in the session. Shown on the score screen as a session summary stat. May contribute a multiplier or flat bonus to final score (to be determined in playtesting).

**Order completion points** Each fulfilled order awards a flat point bonus, with bonus value scaling to the order's target tier.

---

## Technical Architecture

### Stack

|Layer|Technology|
|---|---|
|Language|Rust|
|Engine|Bevy (3D)|
|Physics|Rapier3D|
|Rendering|Bevy PBR, isometric orthographic camera|
|Build target|WASM (via wasm-bindgen / trunk)|
|Distribution|itch.io|

### Physics Notes

- Spheres are the simplest Rapier3D collision primitive — native support, no convex decomposition required
- Physics live in genuine 3D; the isometric view is achieved via an orthographic camera at a fixed isometric angle (not a 2D perspective trick)
- Friction and restitution coefficients are the primary tuning surface for game feel; sliding behavior, settling speed, and nudge responsiveness all emerge from these values
- Table boundaries are static colliders (sides and far end); spheres bounce and decelerate to rest against them

### Sphere Rendering

- 3D sphere meshes with PBR material; base color corresponds to tier color
- Tier number rendered as a texture overlay on the sphere surface
- Colorblind mode: high-contrast number labels become primary visual indicator

---

## Open Design Questions (Playtesting)

The following values are intentionally left as tuning targets rather than hard specifications:

- Exact radius multiplier per tier
- Dispensing weight distribution across tiers 1–5
- Order difficulty escalation curve and tier weight schedule
- Per-merge point values and order completion bonuses
- Peak tier bonus treatment (multiplier vs. flat bonus vs. summary stat only)
- Friction and restitution coefficients for desired physics feel

---

## Development Phases (Proposed)

**Phase 1 — Physics prototype** Table, static camera, sphere dispensing, Rapier3D friction/collision. Goal: validate that the shuffleboard feel matches the target.

**Phase 2 — Core loop** Merge logic, chain reactions, tier hierarchy, loss condition.

**Phase 3 — Orders and scoring** Order system, dispensing weights, scoring implementation.

**Phase 4 — Presentation** Sphere materials and colors, tier label overlays, colorblind mode, UI (preview, order display, score).

**Phase 5 — Polish and ship** WASM build, itch.io deployment, playtesting tuning pass on all open design values.

---

## Appendix: Technical Constraints for AI Implementation

### Physics Constraints

- **Dimension Locking:** Spheres must behave as sliding pucks on a 2D plane within the 3D world space. Lock the Y-axis (vertical) translation and lock all X, Y, Z rotations in Rapier3D to prevent rolling, stacking, or airborne bouncing.
- **Friction Baseline:** Set table friction to 0.5 and sphere restitution (bounciness) to 0.3 as baseline initialization values.

```rust
// Suggested initialization values for the AI agent
let mutual_restitution = 0.15; // Low bounciness: absorbs impacts
let contact_friction = 0.3;   // Low-mid friction: allows smooth sliding along walls
let linear_damping = 1.5;     // High damping: simulates table drag so balls settle quickly
```

### Merge Resolution Algorithm

- **Positioning:** A merged sphere must spawn at the exact midpoint between the two parent spheres.
- **Immediate Deletion:** If a merged sphere matches the current Order Tier, it must trigger an item-collected particle/UI event and be despawned from the physics world at the end of the current frame, before the next physics tick.
- **Tie-Breaking:** If a newly spawned sphere overlaps multiple valid same-tier spheres simultaneously, it must merge with the closest one first.

### Baseline Tuning Values (Placeholders)

To ensure the project compiles and runs immediately, use the following initial values:

- **Radius Multiplier:** `1.3` (Tier 1 radius = `0.5` units).
- **Spawn Weights (Tiers 1-5):** Tier 1: 35%, Tier 2: 30%, Tier 3: 20%, Tier 4: 10%, Tier 5: 5%.
- **Initial Orders Pool:** Tiers 3 through 6.
- **Scoring:** `Points = Resulting_Tier * 100`. Order Completion Bonus = `Target_Tier * 500`.

Do these values sound wrong based on the design document? Stop to discuss with the user and explain what you would want to change and why.
