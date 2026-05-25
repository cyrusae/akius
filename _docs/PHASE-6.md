# Phase 6: Presentation, UI & WASM Optimization

Bring the game to life visually. This phase produces the first fully playable build
in both native and browser (WASM) targets.

**Status at phase start:** 15/15 tests passing, WASM target compiles clean.

---

## Context / Key Decisions

| Topic | Decision |
|---|---|
| Sphere rendering | Separate `VisualPlugin` using `OnAdd<Sphere>` observer — physics tests unaffected |
| Tier labels | Floating 3D world-space billboard text above each sphere |
| Color palette | Warm perceptual gradient: violet → sky blue → lime → orange → gold (13 tiers) |
| Arena geometry | Visible flat table surface + low side walls |
| Colorblind mode | Phase 6, triggered by keyboard `C` key **and** visible UI button |
| Colorblind visuals | Distinct per-tier pattern/icon overlaid on sphere surface |
| HUD | Score + current order target + next-sphere-in-queue preview |
| Preview sphere | Shows tier color at normal opacity; no special blocked state |
| Loss boundary | Dashed line across full arena width at the launch/loss Z position |
| Obstruction blocking | No distinct visual needed — if a sphere is at the launch Z, the game is already lost |

---

## Architecture Overview

### New files
- `src/visuals.rs` — `VisualPlugin`: sphere mesh/materials, arena geometry, billboard labels, colorblind toggle
- `src/hud.rs` — `HudPlugin`: score, order, next-sphere, colorblind button

### Modified files
- `src/game_state.rs` — add `ColorblindMode(bool)` resource; add arena dimension fields to `GameSettings`
- `src/main.rs` — register `VisualPlugin` + `HudPlugin`; remove inline `setup` fn (superseded by `VisualPlugin` Startup)
- `src/launcher.rs` — remove hardcoded preview sphere material (now managed by `VisualPlugin`)

---

## Proposed Changes (Detail)

### `src/game_state.rs`
- Add `ColorblindMode(bool)` resource (default: `false`)
- Add to `GameSettings`: `arena_half_width: f32`, `arena_depth: f32`, `wall_height: f32`

---

### `src/visuals.rs` — `VisualPlugin`

**Constants / helpers:**
- `TIER_COLORS: [Color; 13]` — warm perceptual gradient array (violet → gold)
- `fn tier_material(tier: u8, colorblind: bool) -> StandardMaterial`

**Resources:**
- `TierMaterials` — pre-computed `Handle<StandardMaterial>` array for normal and colorblind modes; rebuilt when `ColorblindMode` toggles

**Startup system:**
- Spawn table floor (thin `Cuboid`, matte material)
- Spawn left / right / back walls (`Cuboid` geometry)
- Spawn dashed loss-boundary line (row of small flat cubes at `launcher_z`)
- Spawn `LauncherPreview` entity (moved from `main.rs`)

**Observer — `OnAdd<Sphere>`:**
- Attach `Mesh3d` + `MeshMaterial3d` child entity using `TierMaterials` handle for tier
- Spawn `Text3d` child billboard entity above sphere displaying tier number

**Update systems:**
- `update_colorblind_toggle` — reads `KeyCode::C` and UI button interaction; toggles `ColorblindMode`; rebuilds `TierMaterials`; updates `MeshMaterial3d` on all `SphereVisual` entities
- `update_preview_material` — keeps `LauncherPreview` material in sync with `DispenserQueue::current` tier

---

### `src/hud.rs` — `HudPlugin`

**Startup system — spawn HUD root:**
- Top-left: **Score** label
- Top-center: **Order** label (e.g. "Make a Tier 4!")
- Top-right: **Next sphere** preview (colored swatch + tier number)
- Bottom-right: **Colorblind toggle button** (text toggles "Colorblind: OFF" / "Colorblind: ON")

**Update systems:**
- `update_score_hud` — re-renders score text when `Score::total` changes
- `update_order_hud` — re-renders order text when `ActiveOrder::target_tier` changes
- `update_next_sphere_hud` — updates swatch from `DispenserQueue::next`
- `handle_colorblind_button` — detects `Interaction::Pressed`, flips `ColorblindMode`

---

### `src/main.rs`
- Declare `mod visuals;` and `mod hud;`
- Register `VisualPlugin` and `HudPlugin`
- Remove the inline `setup` fn

---

## Task Checklist

### Preparation
- [x] Add `ColorblindMode(bool)` resource to `game_state.rs`
- [x] Add arena dimension fields to `GameSettings` in `game_state.rs`
- [x] Update `GameSettings::default()` with sensible arena values

### `src/visuals.rs`
- [x] Create file; define `SphereVisual` component, `TierMaterials` resource
- [x] Implement `TIER_COLORS` — 13-tier warm gradient palette
- [x] Implement `tier_material()` helper (normal + colorblind variants)
- [x] Implement `VisualPlugin::build()`
    - [x] Register `TierMaterials` resource
    - [x] Add Startup system: arena floor, walls, dashed loss line, preview sphere
    - [x] Add `OnAdd<Sphere>` observer: attach mesh + material child + `Text3d` billboard (implemented as camera-facing `Text2d` billboard)
    - [x] Add Update system: `update_colorblind_toggle` (KeyCode::C + button)
    - [x] Add Update system: `update_preview_material`

### `src/hud.rs`
- [x] Create file; define HUD marker components
- [x] Implement `HudPlugin::build()`
    - [x] Startup: spawn HUD root with score / order / next-sphere / colorblind button nodes
    - [x] Update: `update_score_hud`
    - [x] Update: `update_order_hud`
    - [x] Update: `update_next_sphere_hud`
    - [x] Update: `handle_colorblind_button`

### `src/main.rs`
- [x] Declare `mod visuals` and `mod hud`
- [x] Register `VisualPlugin` and `HudPlugin`
- [x] Remove inline `setup` fn

### `src/launcher.rs`
- [x] Remove hardcoded preview sphere spawn / material from `LauncherPlugin` (now in `VisualPlugin`)

### Verification
- [x] `cargo test` — all 15 tests still pass
- [x] Native build: `cargo build` compiles clean without errors
    - [x] Spheres appear in correct tier colors with number labels
    - [x] Aiming preview sphere tracks mouse and shows current tier color
    - [x] Launching spawns sphere that rolls forward and merges on contact
    - [x] HUD shows live score, order, and next-sphere preview
    - [x] `C` key toggles colorblind mode; all sphere materials update
    - [x] Colorblind button toggles state and updates button label
    - [x] Dashed loss boundary line visible at correct Z position
- [x] WASM: compiles clean for `wasm32-unknown-unknown` target
- [x] WASM release: `cargo build --target wasm32-unknown-unknown --release` — completes without errors

---

## Key Bevy 0.18 Notes (for session resume)

- **Component-based spawning** — use `.spawn((Mesh3d(h), MeshMaterial3d(m), Transform::…))` not bundles
- **Observers** — `app.add_observer(on_sphere_added)` where `fn on_sphere_added(trigger: Trigger<OnAdd, Sphere>, …)`
- **`Query::single` / `single_mut`** return `Result`, not panic — always `let Ok(x) = q.single() else { return; }`
- **`ReadRapierContext::single()`** — same Result pattern
- **Physics tests** — use `TransformPlugin + TimePlugin + RapierPhysicsPlugin` (NOT `MinimalPlugins`) for any test involving Rapier
- **`bevy_rapier3d` features** — currently `default-features = false, features = ["dim3", "simd-stable"]` to avoid asset/scene resource deps in headless tests
