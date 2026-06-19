# Crate Architecture Reference

This document describes the Cargo Workspace subcrate structure of the **akius** project. Reorganized under the Rust 2024 edition, this multi-crate layout isolates core game rules and physics from Bevy rendering systems, facilitating headless simulation/training while preserving a single source of truth.

---

## 1. Directory Structure

```text
akius/
├── Cargo.toml                  # Workspace root configuration
├── Cargo.lock                  # Shared workspace lockfile
├── index.html                  # Main entry page (targets akius_game)
├── assets/                     # Shared game assets (audio, fonts, textures, shaders)
│
├── akius_core/                 # 1. CORE SIMULATION LIBRARY
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs              # AppState, Score, Sphere, checking & reset systems
│       ├── core_math.rs        # Radii math, scoring logic, queue random weights
│       └── physics_rules.rs    # Rapier3D physics systems, collision/merge rules
│
├── akius_game/                 # 2. PLAYABLE BEVY GAME FRONTEND
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # Bevy app bootstrap, setup camera/lighting
│       ├── launcher.rs         # Launcher aiming, mouse, touch input systems
│       ├── visuals.rs          # Shaders, camera, lighting, effects, high scores
│       ├── hud.rs              # UI layouts & responsive stacking
│       └── utils.rs            # Change-detection visibility helper
│
└── akius_train/                # 3. HEADLESS ML ENVIRONMENT & BINDINGS
    ├── Cargo.toml
    └── src/
        └── main.rs             # Headless training loop/Gymnasium bindings
```

---

## 2. Crate Responsibilities

### `akius_core` (Core Library)
* **Goal**: Runs the headless game state machine and physics simulation as fast as possible.
* **Dependencies**: Compiles Bevy without default features (`default-features = false`). It pulls in only core ECS features (`bevy_app`, `bevy_ecs`, `bevy_state`, `bevy_log`). It has no dependency on graphics windowing, rendering, UI, or audio libs.
* **Key Components**:
  * `core_math`: Target scoring math, sphere radius calculations ($R(n) = 0.5 \times 1.21^{n-1}$), and dispenser random weights.
  * `physics_rules`: Physics configuration (density, friction, restitution, locked axes) and merge/collision detection logic using Bevy Rapier 3D.
  * Game resources (`Score`, `ActiveOrder`, `DispenserQueue`, `ActiveFulfillment`).

### `akius_game` (Playable Frontend)
* **Goal**: Displays the game interface, handles user inputs, and plays audio.
* **Dependencies**: Bevy with full rendering, windowing, UI, and audio features (`bevy_render`, `bevy_ui`, `bevy_audio`, etc.) + `bevy_mesh`, and depends on `akius_core`.
* **Key Components**:
  * `launcher`: Aiming guide lines, keyboard movement, screen-to-world raycasting, and obstruction checks.
  * `visuals`: Color palettes, outer shells, particle systems, CRT post-processing, and local storage high score writes.
  * `hud`: UI rendering, layout responsiveness, orientations stacking, and buttons/labels sizing.

### `akius_train` (Machine Learning Target)
* **Goal**: Headless simulation runner and Python Gymnasium wrapper entry point.
* **Dependencies**: Depends on `akius_core` (and optional PyO3 dependencies for Python bindings). Bypasses all `akius_game` code.

---

## 3. Configuration & Compilation Details

### Rust Edition
All crates in the workspace use the **Rust 2024** edition.

### The Bevy `Sphere` Namespace Collision
Because Bevy 0.15+ introduces a built-in 3D primitive called `bevy::prelude::Sphere`, any code importing `bevy::prelude::*` will shadow our game's custom `Sphere` component.
* **Solution**: In all game files, our custom `Sphere` component is explicitly imported *after* the Bevy prelude:
  ```rust
  use bevy::prelude::*;
  use akius_core::Sphere; // Explicitly override the primitive Sphere
  ```

---

## 4. Test Suite Division

Tests are distributed based on dependency constraints to avoid circular reference issues:

### `akius_core` Tests (14 Tests)
Validates core mechanics, math, and physics logic in isolation:
* Math & Scoring (`test_radius_scaling`, `test_scoring_math`, `test_weighted_dispenser_distribution`)
* Rules & Fulfilment (`test_order_fulfillment`, `test_loss_condition_recoil`, `test_loss_condition_fallen_y`, `test_loss_condition_grace_period`, `test_restart_from_game_over`)
* Physics Attributes (`test_sphere_spawn_physics_attributes`, `test_dampen_rebound_velocity`, `test_spill_spheres`, `test_double_merge_safety`, `test_collision_midpoint_merge`, `test_merge_momentum_conservation`)

### `akius_game` Tests (13 Tests)
Validates inputs, HUD behaviors, and rendering updates:
* Inputs & Cooldowns (`test_raycast_cursor_plane_intersection`, `test_launch_spawn_and_impulse`, `test_merge_cooldown_blocking`, `test_launch_cooldown_blocking`, `test_touch_start_origin_filter`, `test_preview_obstruction_detection`)
* UI & Render settings (`test_in_game_over_or_win_state`, `test_crt_screen_position_inverts_shader_curve`, `test_crt_screen_position_pulls_toward_center`, `test_label_projection`, `test_auto_degrade_fires_on_sustained_slowness`, `test_auto_degrade_ignores_healthy_and_spiky_frames`)
* Leak checks (`test_entity_count_soak` - checks visuals/labels cleanups)

---

## 5. Deployment / Trunk Integration

To deploy the game to GitHub Pages or Itch.io:
1. The `index.html` file at the root points Trunk to the subcrate's manifest:
   ```html
   <link data-trunk rel="rust" href="akius_game/Cargo.toml" />
   ```
2. Build the project using Trunk from the workspace root:
   ```bash
   trunk build --release
   ```
3. Zip or deploy the `/dist` directory. The compiled WebAssembly output (`akius_game.wasm`) will contain both `akius_game` and its statically linked `akius_core` library dependency.
