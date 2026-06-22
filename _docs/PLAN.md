# akiuS Game — Test-Driven Development (TDD) & CI Plan

This document outlines the step-by-step implementation plan for **akiuS**, a physics-based merge puzzle game built with Bevy and Rapier3D. The plan is designed around Test-Driven Development (TDD) principles, ensuring that systems, logic, and state transitions are verified by unit and integration tests from day one.

---

## Testing Milestones & Human-in-the-Loop Testing

To maximize development velocity and avoid premature manual playtesting (which is slow and error-prone), we divide verification into distinct zones:

| Phase | Primary Testing Type | Meaning of Tests / Focus | Human-in-the-Loop Needed? |
|---|---|---|---|
| **Phase 2 (Math)** | **Unit Tests** | Pure logic functions (scaling, points, random distributions). | ❌ No (fully automated) |
| **Phase 3 (ECS State)** | **ECS Integration Tests** | Bevy system updates, time counters, AppState transitions. | ❌ No (automated via mock Bevy app) |
| **Phase 4 (Physics/Merge)** | **Physics Integration Tests** | Collision event simulation, entity spawning/despawning, momentum inheritance. | ❌ No (automated via headless collision injection) |
| **Phase 5 (Input/Launch)** | **Interactive/Manual Testing** | Raycasting mouse controls, preview collision blocking, click-to-shoot responsiveness. | **Yes** (First playable milestone; game-feel tuning, slide friction, damping) |
| **Phase 6 (UI/WASM)** | **Visual/Manual Testing** | Colorblind label readability, UI settings toggle, browser rendering, packaging size. | **Yes** (UI alignment, graphic fidelity, setting toggles) |

---

## Testing Strategy in Bevy (ECS)

In an ECS architecture, tests are split into two categories:
1. **Pure Logic Tests:** Standard Rust unit tests targeting pure functions (e.g., radius scaling, scoring equations, random weight selection).
2. **ECS Integration Tests:** Spin up a minimal Bevy `App`, register components, resources, and the target systems, execute `app.update()`, and assert changes to the world state.

### ECS Test Template Reference
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[test]
    fn test_system_behavior() {
        // 1. Initialize a minimal App
        let mut app = App::new();
        
        // 2. Insert test resources
        app.insert_resource(ScoreResource { score: 0 });
        
        // 3. Spawn test entities
        let entity = app.world_mut().spawn((Sphere { tier: 1 }, InsideLauncher)).id();
        
        // 4. Register the system under test
        app.add_systems(Update, my_system_to_test);
        
        // 5. Execute one frame tick
        app.update();
        
        // 6. Assert expected side effects
        let score = app.world().resource::<ScoreResource>();
        assert_eq!(score.score, 100);
    }
}
```

---

## Phase 1: Project Bootstrapping & CI Pipeline

Establish the environment, dependencies, target configurations, and a automated validation pipeline.

### Step 1.1: Dependency & Target Config
*   **Action:** Create `Cargo.toml`, `.cargo/config.toml`, and `Trunk.toml` with Bevy 0.18.1, bevy_rapier3d 0.34.0, rand 0.9, and WASM JS targets.
*   **Verification:** Run `cargo check` and `cargo check --target wasm32-unknown-unknown` to confirm dependency resolution.

### Step 1.2: CI/CD Pipeline Configuration
*   **Action:** Create `.github/workflows/ci.yml` to automatically execute:
    *   `cargo fmt --all -- --check` (Code style validation)
    *   `cargo clippy --all-targets -- -D warnings` (Linter validation)
    *   `cargo test` (Host architecture unit/integration tests)
    *   `cargo build --target wasm32-unknown-unknown` (WASM target build validation)
*   **Verification:** Commit files and verify a successful run on GitHub Actions.

---

## Phase 2: Core Data Types & Game Math (TDD)

Build and test the pure logic math libraries before spawning any Bevy systems.

### Step 2.1: Sphere Tiers & Radius Calculations
*   **Test Cases:**
    *   Verify Tier 1 has a radius of `0.5`.
    *   Verify Tier 13 has a radius of `4.92` (using `1.21` multiplier).
    *   Verify scaling calculations handle out-of-bounds inputs gracefully.
*   **Implementation:** Implement the `Tier` enum/struct and a mathematical function `get_radius(tier: u8) -> f32`.

### Step 2.2: Scoring Math
*   **Test Cases:**
    *   Verify `Points = Resulting_Tier * 100`.
    *   Verify order completion bonus scales correctly.
*   **Implementation:** Implement pure scoring formulas.

### Step 2.3: Weighted Dispenser Randomization
*   **Test Cases:**
    *   Verify distribution percentages with a mock RNG (using fixed seeds).
    *   Ensure tiers 6+ are never generated.
*   **Implementation:** Implement the sphere dispenser selection logic using the `rand` crate.

---

## Phase 3: Game State & Order Systems (TDD)

Introduce the Bevy ECS resources, components, and systems managing game state transitions and orders.

### Step 3.1: Active Order Progression
*   **Test Cases:**
    *   Verify game starts with an order drawn from the initial pool (tiers 3-6).
    *   Verify that triggering a completion event despawns the target sphere, updates the score, and generates a new order.
*   **Implementation:** Build the `OrderResource` and the order evaluation system.

### Step 3.2: Loss Line Validation
*   **Test Cases:**
    *   Verify that a sphere crossing the Z threshold triggers `AppState::GameOver` only after remaining there for `> 1.5` seconds.
    *   Verify that spheres tagged with `InsideLauncher` do not trigger loss, even if they sit behind the threshold.
    *   Verify that a sphere momentarily passing through but bouncing back out does not trigger loss.
*   **Implementation:** Implement the loss detection system utilizing a tracking timer or `Time` resource.

---

## Phase 4: Physics & Merging Mechanics (TDD)

Integrate `bevy_rapier3d` and implement the merge resolution loop. This phase uses integration tests that mock Rapier collision events.

### Step 4.1: RigidBody & Dimension Locking Setup
*   **Test Cases:**
    *   Verify launched spheres have Y-translation locked.
    *   Verify spheres have X, Y, Z rotational axes locked.
*   **Implementation:** Add systems to apply Rapier `LockedAxes` and dynamic rigid bodies to spheres.

### Step 4.2: Collision and Midpoint Merging
*   **Test Cases:**
    *   Verify two same-tier spheres (e.g., Tier 1) in contact are despawned.
    *   Verify a Tier 2 sphere is spawned at the exact midpoint.
    *   Verify double-despawn safety: if three spheres touch simultaneously, only two merge, leaving the third intact without crashing.
*   **Implementation:** Implement the collision event listener system and the subsequent merge resolution system (using a safe staging queue).

### Step 4.3: Merge Momentum Conservation
*   **Test Cases:**
    *   Verify the newly spawned sphere inherits `0.5 * (parent_a.velocity + parent_b.velocity)`.
*   **Implementation:** Read velocities from parent entities' `Velocity` components before despawning and apply the scaled average to the new sphere.

---

## Phase 5: Aiming, Raycasting & Launching (TDD/Manual)

Implement player interaction and input mapping.

### Step 5.1: Raycasting to Table Plane
*   **Test Cases:**
    *   Verify 2D cursor positions map to correct 3D table coords on $Y = 0$.
*   **Implementation:** Raycast from the isometric camera into a horizontal `Plane` to determine the X coordinate.

### Step 5.2: Preview and Obstruction Check
*   **Test Cases:**
    *   Verify preview sphere snaps to the raycasted X coordinate while Z remains fixed.
    *   Verify that if the preview sphere overlaps a settled sphere, launch is disabled and the preview color changes.
*   **Implementation:** Cast a shape query (e.g., `RapierContext::intersection_with_shape`) using the preview sphere's collider to check for obstructions.

### Step 5.3: Straight Launch impulse
*   **Test Cases:**
    *   On mouse click, verify sphere is spawned at current preview X coordinate and receives a fixed forward velocity impulse (negative Z direction).
*   **Implementation:** Spawn sphere, remove `InsideLauncher` component, and apply a Rapier `ExternalImpulse` or direct `Velocity`.

---

## Phase 6: Presentation, UI, & WASM optimization

Polishing visual components and ensuring web readiness.

### Step 6.1: Spheres, Materials, and Billboards
*   **Action:** Apply PBR materials matching tier colors. Spawn child 2D text elements or billboard nodes displaying the tier number directly above the spheres.
*   **Verification:** Ensure labels stay flat relative to the camera viewpoint regardless of camera angle.

### Step 6.2: Colorblind Mode Toggle
*   **Action:** Implement settings resource/UI toggle to switch labels to a high-contrast format.
*   **Verification:** Write a test validating that updating the toggle resource triggers style updates on the labels.

### Step 6.3: WASM Production Build Optimization
*   **Action:** Build the final WASM target via `trunk build --release`. 
*   **Verification:** Verify size optimizations are active and confirm the game loads successfully in a standard web browser on local port `8080`.
