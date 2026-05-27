# Phase 1 Code Review — Adversarial Findings

> Scope: full codebase as-is before any UI phase work.  
> Audience: high-level explanations for the author + actionable agent instructions.  
> Findings ranked: **Bugs → Dead/Unused Code → Architecture → Testing Gaps → Simplification**

---

## 1. Out-of-Bounds Array Access in `update_next_sphere_hud`

**Severity: Bug / Panic**

**What's happening:** `TIER_COLORS` is a 9-element array (indices 0–8). In
`hud.rs:341`, the index is clamped with `.min(9)` instead of `.min(8)`.

```rust
// hud.rs line 341
let idx = (queue.next as usize).saturating_sub(1).min(9); // ← should be .min(8)
let color = TIER_COLORS[idx]; // panics if idx == 9
```

If `queue.next` is ever 11 or higher (which can't happen today since dispensed
tiers cap at 5, but becomes possible the moment you expand the dispenser),
`TIER_COLORS[9]` panics at runtime. Compare to `material_for_tier` in
`visuals.rs:74` which correctly uses `.min(8)`. One copy got the right number;
the other didn't.

**Agent fix:** Change `hud.rs:341` to `.min(8)`.

---

## 2. `InsideLauncher` Is Never Inserted — Component Is Dead Code

**Severity: Bug (silent logic gap) + dead code**

**What's happening:** `InsideLauncher` is defined, tested, and used as a query
filter in three separate systems (`check_loss_condition`, `detect_collisions`,
`update_launcher_aiming`), but it is **never inserted on any entity** in
gameplay code. The filters are therefore no-ops. This was presumably meant to
mark the physics sphere while it's being aimed at the launcher line, but the
design was changed to use a separate visual-only `LauncherPreview` entity
instead — and `InsideLauncher` was never removed.

Concrete consequence: the exclusion filter in `check_loss_condition` has no
effect. Any sphere spawned at `launcher_z` would start triggering the loss
timer immediately (the 0.5s grace period rescues it in practice, but only
accidentally).

**Agent fix:**
- Remove the `InsideLauncher` struct from `game_state.rs`.
- Remove all `Without<InsideLauncher>` filter clauses from query tuples in
  `check_loss_condition`, `detect_collisions`, and `update_launcher_aiming`.
- Remove the launcher-exclusion test `test_loss_condition_launcher_exclusion`
  (it tests a component that no longer exists).

---

## 3. `despawn_recursive_custom` Is Duplicated Across Two Modules

**Severity: Code smell / maintenance hazard**

**What's happening:** Identical private functions are copy-pasted verbatim in
`game_state.rs:207–218` and `hud.rs:467–478`. Any bug fix or behavioral change
must be applied twice.

```rust
// Exact same code in both files:
fn despawn_recursive_custom(commands: &mut Commands, entity: Entity, children_query: &Query<&Children>) {
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            despawn_recursive_custom(commands, child, children_query);
        }
    }
    commands.entity(entity).despawn();
}
```

Bevy's `EntityCommands::despawn()` in current Bevy 0.18 despawns only the
entity itself — children are orphaned unless removed separately. The custom
recursive walk is legitimate. But it should live in one place.

**Agent fix:**
- Create `src/utils.rs` (or add a public function to an existing module) with
  `pub fn despawn_recursive(commands: &mut Commands, entity: Entity, children_query: &Query<&Children>)`.
- Replace both call sites with the shared function.
- Add `mod utils;` to `main.rs`.

---

## 4. `material_for_tier` Takes a `_colorblind` Parameter It Never Uses

**Severity: Unused code / misleading API**

**What's happening:** The function signature is:

```rust
// visuals.rs:70
pub fn material_for_tier(tier: u8, _colorblind: bool, tier_mats: &TierMaterials) -> Handle<StandardMaterial> {
```

The `_colorblind` argument is silently ignored (the `_` prefix is a Rust
convention meaning "intentionally unused"). Every call site passes `false` or
`cb` but nothing changes. The colorblind experience today is handled entirely
through 2D text label visibility, not material switching.

Keeping this parameter creates a false promise: callers think they can request
a colorblind material variant; they can't.

**Agent fix:**
- Remove the `_colorblind: bool` parameter from `material_for_tier`.
- Update the two call sites: `on_sphere_added` (visuals.rs:243) and
  `update_preview_material` (visuals.rs:294).
- Document this decision with a comment if colorblind material variants are
  planned for a future phase.

---

## 5. `ColorblindMode` Is Registered Twice

**Severity: Minor redundancy**

**What's happening:**

```rust
// main.rs:30
.insert_resource(game_state::ColorblindMode::default())

// visuals.rs VisualPlugin::build:
.init_resource::<ColorblindMode>()
```

`init_resource` is a no-op when the resource already exists, so the
`VisualPlugin` call silently does nothing. This is harmless but confusing: a
reader of `VisualPlugin` would assume it owns the resource, and a reader of
`main.rs` would assume the same.

**Agent fix:** Remove the `init_resource::<ColorblindMode>()` line from
`VisualPlugin::build` in `visuals.rs`. Ownership of global game resources
belongs in `main.rs`.

---

## 6. `update_launcher_preview_visuals` Is Outside the `InGame` State Guard

**Severity: Inconsistency / fragility**

**What's happening:** In `LauncherPlugin::build`, three systems are correctly
gated on `AppState::InGame`:

```rust
(update_launcher_aiming, check_launcher_obstructions, handle_launch_input)
    .chain()
    .run_if(in_state(AppState::InGame)),
update_launcher_preview_visuals,   // ← runs in ALL states
```

`update_launcher_preview_visuals` compensates by manually checking the state
inside its body and hiding itself when not in-game. This works, but it means
the system runs every frame even during menus and game-over screens, and it
reads four queries + a resource every frame to do nothing. It also means the
logical gate is expressed in two different places.

**Agent fix:** Move `update_launcher_preview_visuals` inside the `.chain()`
with the other systems and add it to the `.run_if(in_state(AppState::InGame))`
guard. Remove the manual `is_in_game` check inside the function body.

---

## 7. `animate_merged_spawns` Duplicates Its Own Loop Body

**Severity: Simplification**

**What's happening:** The function contains two nearly-identical loops — one
over `visual_query` (mesh transforms) and one over `label_query` (text
transforms) — that compute the same `t`/`scale` value from the same
`MergeCooldown` timer:

```rust
// Repeated twice, once per query type:
if let Ok(cooldown) = cooldown_query.get(parent) {
    let t = (elapsed / duration).clamp(0.0, 1.0);
    let scale = 0.8 + t * 0.2;
    transform.scale = Vec3::splat(scale);
} else {
    if transform.scale != Vec3::ONE { transform.scale = Vec3::ONE; }
}
```

`animate_fulfilling_spheres` has the same structure.

**Agent fix:** Extract a helper:
```rust
fn compute_merge_scale(cooldown: &MergeCooldown) -> f32 {
    let t = (cooldown.timer.elapsed_secs() / cooldown.timer.duration().as_secs_f32()).clamp(0.0, 1.0);
    0.8 + t * 0.2
}
```
Then unify both query loops in `animate_merged_spawns` by looking up the
scale once per parent entity and applying it to both the mesh child and the
label. Same refactor for `animate_fulfilling_spheres`.

---

## 8. O(N²) Occlusion Check in `update_labels_screen_position`

**Severity: Performance / future scalability**

**What's happening:** For every label (one per sphere), the function does a
full linear scan over all other spheres to determine if the label is occluded:

```rust
// visuals.rs:346
for (other_entity, other_transform, other_sphere) in sphere_query.iter() { ... }
```

With N spheres on the board, this is O(N²) work per frame. At the current
scale (max ~15–20 spheres before game-over), this is not a performance problem
today. But it's the single most expensive Update system and will be the first
thing to profile badly when sphere counts grow.

The analytical ray–sphere approach is elegant; the O(N²) structure is not.

**Agent fix (Phase 1 — cleanup without rearchitecting):**
- Cache the sphere positions in a `Vec` before the outer loop to avoid
  repeated ECS iteration overhead.
- Add a comment marking this as the known hotspot.

**Agent fix (Phase 2 — correct approach):**
- Replace the occlusion check with a simple depth-sort: label the sphere whose
  screen-space depth (NDC z) is smallest (closest to camera); skip labels
  behind others. This can be done in one O(N log N) sort pass.

> **COMMENT:** *I'm not sure abouot this one, because of the risk of a sphere that's large blocking multiple smaller ones.*

---

## 9. `check_order_fulfillment` Weight Formula Is Opaque

**Severity: Readability / maintenance hazard**

**What's happening:** The tier selection logic works correctly, but the
construction of tier-7's probability as an inline expression is hard to audit:

```rust
// game_state.rs:177
} else if roll < w5 + w6 + (100 - w5 - w6 - w8) {
    7
```

`100 - w5 - w6 - w8` *is* tier 7's weight (the remainder), but it's never
named, and the three-level if/else chain obscures the boundary math. The
comment block above it accurately describes the intent — but anyone editing
the weights must mentally re-derive that none of the four weights sum past 100.

Additionally, `(60 as u32)` is an unnecessary cast; `60u32` is idiomatic Rust.

**Agent fix:**
```rust
let w8 = (score.completed_orders * 10).min(40);
let w6 = 60u32.saturating_sub(score.completed_orders * 10).max(15);
let w5 = 5u32;
let w7 = 100 - w5 - w6 - w8;  // ← named remainder

let roll = rand::random_range(0..100u32);
active_order.target_tier = if roll < w5 {
    5
} else if roll < w5 + w6 {
    6
} else if roll < w5 + w6 + w7 {
    7
} else {
    8
};
```

---

## 10. Visibility Guard Pattern Repeated ~15 Times

**Severity: Simplification**

**What's happening:** Throughout `visuals.rs` and `launcher.rs`, the same
guarded-set pattern appears repeatedly:

```rust
if *visibility != Visibility::Hidden {
    *visibility = Visibility::Hidden;
}
```

This idiom exists to avoid triggering change detection when the value isn't
actually changing (a valid Bevy optimization). But writing it inline every time
creates visual clutter and is easy to write wrong (e.g., checking Hidden but
setting Visible by accident).

**Agent fix:** Add two helper functions (e.g., in `utils.rs`):
```rust
pub fn set_visibility(vis: &mut Visibility, target: Visibility) {
    if *vis != target { *vis = target; }
}
```
Then replace all inline guard patterns with `set_visibility(&mut visibility, Visibility::Hidden)`.

---

## 11. Testing Gaps

### 11a. `test_weighted_dispenser_distribution` Tests a Tier That Cannot Exist

`core_math.rs:79` asserts `tier >= 1 && tier <= 5`, but `get_random_dispensed_tier`
only returns 1–4 (`_ => 4`). Tier 5 is unreachable. The bounds check in the
test is too wide and will silently accept a broken version of the function that
returns 5.

**Agent fix:** Change the assert to `tier >= 1 && tier <= 4`.

### 11b. No Test for `spill_spheres`

`spill_spheres` in `physics.rs` is the system that unlocks Y-translation and
rotation when a sphere crosses `launcher_z`. It has no unit test. This is one
of the trickier systems (depends on the Rapier context being present) but the
axis-unlocking behavior is testable at the component level without a full
physics simulation.

**Agent test sketch:**
- Spawn a sphere with full `LockedAxes` at z = 0, then teleport it to
  `z > launcher_z`. Run one update. Assert `LockedAxes` is now empty (or no
  longer present).

### 11c. No Test for Merge-Block During Active Cooldown

`handle_launch_input` blocks firing when `!merge_cooldowns.is_empty()`. The
existing `test_launch_cooldown_blocking` tests the timer-based cooldown but
not the merge-cooldown gate. A sphere mid-merge should prevent a launch even
if the cooldown timer has expired.

**Agent test sketch:**
- Set up an app with `handle_launch_input`, advance time past 0.8s cooldown.
- Spawn an entity with `MergeCooldown`.
- Press left-click. Assert no sphere is spawned.

### 11d. No Test for `check_order_fulfillment` Timer Reset Behavior

The fulfillment timer is stored in `ActiveFulfillment` and reset via
`fulfillment.timer.reset()` when a new match is found. There is no test that
verifies re-triggering: if the fulfilling entity is despawned externally (e.g.,
via a merge) while fulfillment is in progress, `fulfillment.entity` still
holds a stale handle and `commands.entity(entity).despawn()` will silently no-op
in Bevy (despawning an already-despawned entity). The score update will still
fire. This should be guarded.

**Agent fix:** Before calling `commands.entity(entity).despawn()` in
`check_order_fulfillment`, check `sphere_query.get(entity).is_ok()` (or use
`commands.get_entity(entity)`) and skip the score update if the entity no
longer exists.

---

## 12. `Fulfilling` Spheres Bypass Distance-Merge Detection But Not Collision Detection

**Severity: Edge-case logic gap**

**What's happening:** `check_distance_merges` excludes `Fulfilling` spheres
from its proximity scan (the `Without<Fulfilling>` filter). But `detect_collisions`
does **not** exclude `Fulfilling` spheres — only spheres `Without<InsideLauncher>`
and `Without<MergeCooldown>`. If a free sphere collides with a fulfilling
sphere of the same tier during the 1.2s fulfillment window, `detect_collisions`
will fire a `MergeEvent` for the fulfilling entity.

`resolve_merges` will then try to despawn the fulfilling sphere via
`DespawnDelay`, which happens after `check_order_fulfillment`'s despawn also
runs. Double-despawn in Bevy is a no-op, but the score is incremented once
(by the fulfillment timer) *and* once more (by the merge points), and a new
upgraded sphere is spawned from a sphere that was supposed to disappear as an
order completion.

**Agent fix:** Add `Without<crate::game_state::Fulfilling>` to the
`detect_collisions` sphere query filter, matching the existing guard in
`check_distance_merges`.

---

## 13. Architecture: Visuals Are Tangled Into Physics Spawn

**Severity: Architecture (future-phase hazard)**

**What's happening:** `spawn_sphere_entity` in `physics.rs` adds
`Visibility::default()` to the physics entity:

```rust
// physics.rs:84
Visibility::default(),
```

This is a rendering concern living in the physics module. It works because
Bevy requires `Visibility` for hierarchy propagation, but it's the thin end of
a wedge that will make headless testing harder (all physics tests must either
carry or ignore rendering components).

More structurally: the `on_sphere_added` observer in `visuals.rs` is the
*correct* decoupling point — visuals react to physics events. But
`spawn_sphere_entity` reaching into rendering concepts (via `Visibility`)
blurs that line.

**Agent fix for Phase 1:**
- Remove `Visibility::default()` from `spawn_sphere_entity`.
- Add it to the `on_sphere_added` observer via `commands.entity(entity).insert(Visibility::default())` if Bevy requires it for 2D overlay label positioning.
- Audit whether it's actually needed (labels use a separate root entity; the
  sphere mesh is a child spawned by the observer, so the parent may not need
  `Visibility` at all).

---

## 14. Win Condition Silently Swallowed if State Resource Is Absent

**Severity: Logic gap / silent failure**

**What's happening:** `resolve_merges` in `physics.rs` declares its state
resource as optional:

```rust
// physics.rs:165
mut next_state: Option<ResMut<NextState<crate::game_state::AppState>>>,
```

This means if `AppState` is not registered in the app (which is the case in
every physics unit test that uses `PhysicsPlugin` directly), triggering the
Tier 9 win condition does nothing — no panic, no warning, just a silent
`if let Some(ref mut ns) = next_state { ... }` branch that is never taken.

The `Option` was likely added to make tests easier: `PhysicsPlugin` can be
added to a test app without also wiring up the full state machine. But it
trades test convenience for a production guarantee. If `AppState` were ever
accidentally unregistered, or if a future refactor moves physics into a
context where the state resource isn't present, the win condition would vanish
without any compile-time or runtime signal.

**Agent fix:**
- Make the resource required: change `Option<ResMut<NextState<AppState>>>` to
  `ResMut<NextState<crate::game_state::AppState>>` in the system signature.
- In the tests that use `PhysicsPlugin` and exercise merges (`test_collision_midpoint_merge`,
  `test_double_merge_safety`, `test_merge_momentum_conservation`), add:
  ```rust
  app.add_plugins(bevy::state::app::StatesPlugin);
  app.init_state::<crate::game_state::AppState>();
  ```
  This is a few lines per test and keeps the production path honest.

---

## 15. `hud.rs` Has No Tests

**Severity: Testing gap**

The HUD module (`hud.rs`) contains all UI layout, button interaction handlers,
and score/order display logic. It currently has zero tests. The HUD is mostly
presentational, but two behaviors are worth testing without a full rendering
stack:

1. **`in_game_over_or_win_state` run condition** — ensure it correctly admits
   `GameOver` and `Win` but rejects `MainMenu` and `InGame`.
2. **`update_score_hud` / `update_order_hud` text change guards** — ensure
   text isn't rewritten every frame when the value hasn't changed (the `!=`
   guard exists; it should be tested).

---

## Summary Table

| # | File | Issue | Kind |
|---|------|-------|------|
| 1 | `hud.rs:341` | `TIER_COLORS` indexed with `.min(9)` instead of `.min(8)` — potential panic | **Bug** |
| 2 | `game_state.rs` / queries | `InsideLauncher` never inserted; all exclusion filters are no-ops | **Bug / Dead code** |
| 3 | `game_state.rs:207`, `hud.rs:467` | `despawn_recursive_custom` copy-pasted verbatim | **Duplication** |
| 4 | `visuals.rs:70` | `_colorblind` parameter never used in `material_for_tier` | **Dead code** |
| 5 | `main.rs:30`, `visuals.rs` | `ColorblindMode` registered twice | **Redundancy** |
| 6 | `launcher.rs` plugin | `update_launcher_preview_visuals` outside `InGame` guard | **Inconsistency** |
| 7 | `visuals.rs:519` | Merge animation loop body duplicated for mesh vs. label | **Duplication** |
| 8 | `visuals.rs:346` | O(N²) occlusion check in label projection | **Performance** |
| 9 | `game_state.rs:168–181` | Order weight formula uses unnamed `100-w5-w6-w8` inline | **Readability** |
| 10 | `visuals.rs`, `launcher.rs` | `if *vis != X { *vis = X; }` pattern repeated ~15 times | **Simplification** |
| 11a | `core_math.rs:79` | Test asserts tier ≤ 5 but function only returns 1–4 | **Test bug** |
| 11b | `physics.rs` | `spill_spheres` has no test | **Test gap** |
| 11c | `launcher.rs` | Merge-cooldown launch block has no test | **Test gap** |
| 11d | `game_state.rs:149–154` | Stale fulfillment entity not guarded before despawn | **Logic gap** |
| 12 | `physics.rs:101` | `detect_collisions` doesn't exclude `Fulfilling` spheres | **Bug** |
| 13 | `physics.rs:84` | `Visibility::default()` in physics spawn function | **Architecture** |
| 14 | `physics.rs:165` | Win condition uses `Option<ResMut<NextState>>` — silently swallowed if resource absent | **Logic gap** |
| 15 | `hud.rs` | Zero tests | **Test gap** |
