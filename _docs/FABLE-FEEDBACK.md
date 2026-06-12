# akiuS — Code Quality & Performance Review

**Date:** 2026-06-11 (review) / updated same day after the fix session
**Focus:** Tab crashes on GH Pages after extended play / when approaching high scores.
**Scope:** Full `src/` review, shaders, build config, and the deployed `dist/` artifact.

---

## Status summary

Almost everything below was fixed in the same session (working tree, uncommitted).
Legend: ✅ fixed · 🟡 mitigated / partially addressed · ⬜ open

| Item | Status |
|---|---|
| P0-1 Per-frame material mutation (laser/reticle) | ✅ `globals.time` in shaders; materials touched only on tier change |
| P0-2 Preview material handle reassigned every frame | ✅ change-guarded |
| P0-3 Verify release deploys | ✅ confirmed always `--release`; deploy is manual `npx gh-pages` (see Remaining) |
| P1-4 WASM heap ratchet (per-frame allocations) | ✅ scratch buffers reused; collider cached per tier |
| P1-5 Physics in all states; game-over spheres fall forever | ✅ state-gated; fallen spheres despawned below y −20 |
| P1-6 Alpha-blend overdraw scales with board fullness | 🟡 FX auto-degrade added; opaque-shell option intentionally not taken |
| P2-7 CRT settings re-inserted every frame | ✅ mutated in place |
| P2-8 Sync localStorage writes while beating high score | ✅ throttled to 5 s + flush on GameOver/Win |
| B1 Fulfillment latches dying merged parent | ✅ `Without<DespawnDelay>` on fulfillment + merge queries |
| B2 Label pop animation dead code (Transform vs UiTransform) | ✅ uses `UiTransform`; animation works again |
| B3 Matrix particles invisible (Sprites without Camera2d) | ✅ converted to shared-asset unlit 3D quads |
| B4 `adjust_camera_fov` dirties Projection every frame | ✅ change-guarded |
| Label lateral drift bug (reported separately) | ✅ root-caused: forward vs **inverse** CRT curve; fixed + tested |
| Q1 Trim Bevy default features | ✅ explicit list; audio kept for future SFX; 22 MB → 19 MB |
| Q3 Repo hygiene | ✅ `cargo_check.log` removed/ignored (`dist/` was already untracked) |
| Q4 Duplicated barrel-distortion math | ✅ single `crt_screen_position()` helper |
| Q5 Duplicated tier-index pattern | ✅ `tier_index()` helper |
| Q6/Q7 Doc-comment dupe, `spill_spheres` Option resource | ✅ |
| Q2 Automated Pages deploy | ⬜ open (manual `npx gh-pages` flow retained) |
| Q8 Entity-count soak test | ⬜ open |

Verification at end of session: clippy clean (0 warnings), 25/25 tests passing,
native + wasm targets compile, `trunk build --release` succeeds. **Pending: one
human visual pass** (`trunk serve`) before next deploy — required because of the
feature trim (render-feature failures are runtime, not compile-time) and to
sanity-check the restored merge-burst particles and label positioning.

---

## TL;DR (original findings)

There was no single "allocates forever" leak left — the earlier Text2d/Camera2d fix
took care of the worst one. What remained was a **death-by-a-thousand-cuts pattern
worst exactly when the board is full (i.e., when chasing a high score)**:

1. **Per-frame material asset mutation** forced Bevy to recreate GPU uniform
   buffers + bind groups every frame, forever (laser line, reticle). On WebGL2 this
   is constant GPU-resource churn and JS-object GC pressure — the classic profile
   of a tab that dies after 30–60 minutes.
2. **WASM heap never shrinks.** Every allocation spike (merge chains, particle
   bursts, O(n²) scratch vectors) permanently ratchets the heap upward.
3. **GPU load scales with board fullness**: two draw calls per sphere, alpha-blended
   shells (no depth write, sorted, heavy overdraw), plus a fullscreen CRT pass —
   worst case is precisely "approaching a high score."

---

## 1. Crash-relevant findings

### ✅ P0-1: Per-frame `Assets::get_mut` on materials recreated GPU resources every frame
**Was:** `update_aim_guide_line` / `update_targeting_reticle` wrote `time` into the
laser/reticle material uniforms every frame. `get_mut` fires `AssetEvent::Modified`,
which re-extracts and re-prepares the material each frame (verified in
`bevy_render-0.18.1/src/render_asset.rs:279`) — allocating a new uniform buffer +
bind group and dropping the old one, 60×/sec for the whole session.

**Fixed:** `laser_line.wgsl` / `reticle.wgsl` now use `globals.time` (the `time` +
padding uniform fields were removed on both the WGSL and Rust sides); the systems
track the last tier in a `Local<Option<u8>>` and only mutate the material on tier
change. Materials are now modified a handful of times per game instead of every frame.

### ✅ P0-2: `update_preview_material` reassigned the material handle every frame
**Fixed:** handle compared before write (matching the core-material path), so
`Changed<MeshMaterial3d>` only fires on real tier changes.

### ✅ P0-3: Deployed build provenance
Confirmed by the maintainer: deploys have always been `trunk build --release`,
published via `npx gh-pages`. `dist/` is untracked. Remaining suggestion: make
`trunk build --release && npx gh-pages -d dist` a single habitual command (or a CI
job — see Remaining) so a stale local `dist/` can never ship.

### ✅ P1-4: WASM heap ratchet — per-frame allocation spikes
**Fixed:**
- `check_distance_merges`: sphere snapshot + merged-set now `Local` scratch buffers.
- `update_launcher_aiming`: interval vectors now `Local` scratch buffers.
- `check_launcher_obstructions`: Rapier query collider cached per tier instead of
  heap-allocated per frame.
- Duplicate doc comment removed while in there.

### ✅ P1-5: Physics ran in every state; game-over spheres simulated forever
**Fixed:** merge detection/resolution/spill systems gated on `InGame`;
`handle_despawn_delay` deliberately left ungated so pending despawns flush across
state changes; new `despawn_fallen_spheres` removes anything below y = −20, so the
game-over spill cleans itself up instead of free-falling (and never sleeping)
while the end screen is open. Physics tests updated to enter `InGame`.

### 🟡 P1-6: Alpha-blend overdraw scales with score
**Mitigated:** `auto_degrade_visual_effects` (in `game_state.rs`) flips FX off after
~3 cumulative seconds of sub-30 FPS gameplay frames; ignores one-off >1 s deltas
(tab switches); fires at most once per session so it never fights a player who
re-enables FX. Covered by two unit tests.

**Deliberately not taken:** switching shells to `AlphaMode::Opaque` (largest raw GPU
win) — the translucent grid look was judged worth keeping on capable hardware.
Revisit only if crash/perf reports continue on weak devices *with FX already off*.

### ✅ P2-7: CRT settings component re-inserted via `Commands` every frame
**Fixed:** mutated in place via `Query<&mut CrtPostProcessSettings>`; insert/remove
now happens only on FX-mode transitions.

### ✅ P2-8: Synchronous localStorage write whenever the high score increased
**Fixed:** in-memory high score still updates every frame, but persistence is
throttled to one write per 5 s while climbing, with `flush_high_score` on
`OnEnter(GameOver)` / `OnEnter(Win)` guaranteeing the final value.

---

## 2. Correctness bugs

### ✅ B1: Order fulfillment could latch onto a dying merged parent
Merged parents keep `Sphere` during their 2-frame `DespawnDelay` window.
**Fixed:** `Without<DespawnDelay>` added to `check_order_fulfillment`,
`detect_collisions`, and `check_distance_merges` — the double-merge protection is
now by construction instead of relying on `resolve_merges` happening to require
`&Velocity`.

### ✅ B2: Label scale animation was dead code
UI nodes carry `UiTransform` in Bevy 0.18 (verified: `Node`'s require list), so the
`Query<&mut Transform>` matched nothing. **Fixed:** both animate systems use
`UiTransform` (`Vec2` scale); the merge/fulfillment label "pop" works again.

### ✅ B3: Matrix particles were invisible
They were `Sprite`s, which stopped rendering when the Camera2d overlay was removed
in `0bc0583`. **Fixed:** converted to camera-facing unlit 3D quads using shared
mesh/material handles (`MatrixParticleAssets` — no per-particle assets), additive
blending, scale-based fade. The burst FX render again for the first time since that
commit; worth a look to confirm the new additive glow reads well.

### ✅ B4: `adjust_camera_fov` dirtied the projection every frame
**Fixed:** writes only when the target fov actually differs.

### ✅ (Reported separately) Number labels drifted laterally with ball position
Root cause: labels applied the CRT shader's `curve()` **forward**, but a
post-process `curve()` maps *output pixel → source sample*, so the rendered scene
is pulled toward screen center while labels were pushed away — ~2× the distortion
of error, growing with distance from center, zero at dead center. **Fixed:**
`crt_screen_position()` applies the exact closed-form inverse (possible because the
shader distorts X first, then derives Y from distorted X). Pinned by a round-trip
test against a reimplementation of the shader curve and a directional regression
test. `BEND = 3.8` must stay in sync with `crt_post_process.wgsl` (documented).

---

## 3. Quality & maintainability

1. ✅ **Bevy features trimmed.** `default-features = false` with an explicit,
   commented list. Dropped: glTF, scenes, animation, gizmos, gamepad, picking
   (UI `Interaction` verified to work without it — `ui_focus_system` is
   unconditional), sprites, SMAA luts, HDR, sysinfo. Kept: `tonemapping_luts` +
   `ktx2` + `zstd_rust` (runtime-failure trap), `default_font`, `webgl2`,
   input features, **and `bevy_audio` + `vorbis` + `wav` for future sound effects**
   (`.ogg`/`.wav` packs will Just Work; `mp3`/`flac` are one-line additions).
   Result: deployed wasm 22 MB → 19 MB (LTO was already stripping much of the
   unused code; the trim also removes per-frame picking/gamepad system overhead and
   guarantees the dead subsystems stay dead). **Re-check the list on Bevy upgrades.**
2. ⬜ **Automated Pages deploy** — still manual `npx gh-pages`. See Remaining.
3. ✅ **Repo hygiene:** `cargo_check.log` deleted and gitignored. (`dist/` and
   `target/` were already correctly untracked, contrary to the original note.)
4. ✅ **Barrel-distortion math deduplicated** into `crt_screen_position()` (which is
   also where the direction bug lived — fixing and deduplicating were the same change).
5. ✅ **`tier_index(tier)` helper** replaced six copies of the saturating clamp pattern.
6. ✅ Duplicated doc comment removed.
7. ✅ `spill_spheres` takes `Res<GameSettings>` directly.
8. ⬜ **Soak test** — see Remaining.

---

## Remaining / hand-off list

Small, well-scoped items suitable for delegation:

1. **Entity-count soak test** (≈30 min). Headless `App` test: run N merge +
   fulfillment + particle-burst cycles, assert world entity count returns to
   baseline. Would have caught the original label leak; guards every future one.
   Suggested home: `physics.rs` or a new `tests/` integration test.
2. **Pages deploy automation** (≈30 min). Either a GH Actions workflow
   (`trunk build --release` → `actions/deploy-pages`) or, lighter, a
   `deploy.sh`/`just deploy` that chains `trunk build --release && npx gh-pages -d dist`.
   Eliminates the stale-`dist/` failure mode permanently.
3. **Glyph-centering constants** (small polish). Labels center via hardcoded
   `-6.5`/`-11.0` px offsets sized for one digit at font 22. Compute from the
   label's `ComputedNode` size (or accept the ~1 px drift; cosmetic only).
4. **Optional, only if weak-GPU reports persist with FX off:** opaque/alpha-mask
   sphere-shell variant (largest remaining GPU lever; visual tradeoff — see P1-6).
5. **Human checks before next deploy:** `trunk serve` visual pass (feature trim,
   particle look, label positions, buttons, touch on a phone), then redeploy and
   confirm with the friends who hit the crashes that long sessions survive.

### Field-verification note
The crash fixes are mechanism-level (churn, heap ratchet, runaway simulation) and
none could be reproduced-to-failure locally in this session. If tabs still die
after extended play on the new build, the next diagnostic step is a heap timeline:
Chrome DevTools → Memory → record while idling on a full board for ~10 minutes;
a still-climbing JS-heap or GPU-process line points at whatever remains.
