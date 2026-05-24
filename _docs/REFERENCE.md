# Game Physics Glossary

## (`akiuS` Reference)

### RigidBody (Dynamic vs. Static)

The core component assigned to an entity that allows it to participate in the physics simulation.

- **Dynamic:** Used for your **spheres**. These bodies respond to forces, gravity, impulses, and collisions.
- **Static:** Used for your **table boundaries and walls**. These bodies do not move or respond to forces, but other objects can collide with or bounce off them.

### Restitution (Bounciness)

The coefficient that determines how much kinetic energy is preserved after a collision. It maps roughly to elasticity.

- **A value of `0.0`:** Perfectly inelastic. The objects will "thud" together and absorb all collision energy, stopping or moving together without bouncing apart.
- **A value of `1.0`:** Perfectly elastic. The objects will bounce away with exactly the same amount of kinetic energy they had going into the collision.

### Contact Friction (Friction Coefficient)

The resistance force encountered when two solid surfaces slide or rub directly against one another.

- **In-Game Behavior:** This determines how much a sphere slows down when it is actively scraping along a side wall, or when two spheres squeeze past each other.
- **Tuning Note:** High contact friction makes walls feel "sticky," causing a launched sphere to lose its forward momentum if it hits a wall at a shallow angle.

### Linear Damping (Surface Drag)

A constant resistive force applied to a rigid body opposite to its direction of travel, independent of contact with other objects. It simulates air resistance or fluid drag.

- **In-Game Behavior:** Since your spheres are sliding across a flat plane without rolling, this is the **primary lever** that simulates table surface friction.
- **Tuning Note:** Increasing this value shortens the distance a sphere travels before naturally coming to a complete stop, giving it a heavy, deliberate shuffleboard or curling feel.

### Axis Locking (Degrees of Freedom)

The restriction of a rigid body's movement or rotation along specific 3D coordinate vectors (X, Y, or Z).

- **Translation Locking:** Disabling movement along an axis. For `akiuS`, locking the vertical axis (typically Y in Bevy/Rapier) confines the spheres to a 2D plane, preventing them from bouncing upward or stacking.
- **Rotation Locking:** Disabling spinning around an axis. Locking all three rotational axes prevents the spheres from rolling like bowling balls, forcing them to slide like pucks so their texture overlays stay facing the camera.

### Collision Normal

The perpendicular mathematical vector branching outward from the exact point where two shapes make contact.

- **In-Game Behavior:** When two spheres collide, the physics engine uses this line connecting their center points to calculate the exact angle and direction of their bounce (planar recoil).
---
## 1. The Bevy ECS Paradigm

Bevy doesn't use traditional Object-Oriented Programming (where a `Sphere` is a class containing its own data and methods). Instead, it splits everything into three distinct pieces:

- **Entities:** A unique, lightweight ID. Think of it as a blank passport number. On its own, it has no data and does nothing.
- **Components:** Raw data structures (Rust `structs`) that you attach to an Entity. For example, you might attach a `Tier(u8)`component, a `Velocity` component, and a `Mesh` component to an Entity ID.
- **Systems:** Plain Rust functions that run every frame. They query for specific combinations of components and manipulate them. For example: _"Find all Entities that have both a `Tier` component and a `Transform` component, and update their scale."_

> 💡 **Why this matters for your agent:** If you want to change how spheres look, you don't tell the agent to "edit the sphere class." You tell it to "modify the System that handles sphere rendering."

## 2. Key Architectural Terms for `akiuS`

### Resources

A Bevy feature used to store **global, unique data** that isn't tied to any single entity on the board.

- **In `akiuS`:** Your current score, the active Order, the upcoming sphere preview queue, and the game state (e.g., MainMenu, Playing, GameOver) will all be stored as Resources.
- **Agent Tip:** If the agent is struggling to pass data between the score UI and the physics engine, remind it to _“Store the score in a global Resource.”_

### States (AppState)

A built-in Bevy tool for managing game loops and menus. Systems can be configured to only run when the game is in a specific state.

- **In `akiuS`:** You will have states like `AppState::InGame` and `AppState::GameOver`. When the loss condition triggers, a system will transition the state to `GameOver`. This instantly pauses the physics simulation and sphere launching systems because Bevy stops running them.

### Events

A way for systems to send messages to each other asynchronously without tightly coupling their code.

- **In `akiuS`:** When two spheres collide and merge, a system will fire a `MergeEvent { tier: 4, position: Vec3 }`.
- **The Power of Events:** Once that event is fired, three completely separate systems can listen for it at the same time:
    1. The **Scoring System** hears it and adds points.
    2. The **Audio System** hears it and plays a pop sound.
    3. The **Order System** hears it to check if the target tier was reached.


### Queries and `With`/`Without` Filters

The method systems use to look up data. Queries can filter entities incredibly precisely.

- **Example:** A system handling the loss boundary might look like this: `Query<&Transform, (With<Sphere>, Without<Launcher>)>`. This tells Bevy to grab the coordinates of everything that is a sphere, but ignore the sphere currently sitting in the player's launcher.

## 3. A Common Rust Trap: "Borrow Checker" Collisions

Because Rust enforces strict memory safety, AI agents frequently run into a compilation error called the **Borrow Checker**trap when dealing with physics loops.

When managing chain merges, the agent might write a system that iterates through all spheres to see if they are touching. If Sphere A and Sphere B need to merge, the agent might try to mutate both spheres at the exact same time inside a single loop. Rust will reject this (`cannot borrow as mutable more than once at a time`).

- **How the agent should solve this:** The best practice in Bevy is to have the collision system merely _tag_ the spheres with a temporary component (like `struct NeedsMerge;`) or push their IDs into an Event queue, and then let a completely separate system handle the actual deletion and spawning in the next frame.

---

## Example logic

```rust
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

// ==========================================
// 1. COMPONENTS & RESOURCES (Data Only)
// ==========================================

#[derive(Component)]
pub struct Sphere {
    pub tier: u8,
}

#[derive(Component)]
pub struct InsideLauncher; // Tag to ignore the ball currently in the queue

#[derive(Resource)]
pub struct GameSettings {
    pub loss_boundary_z: f32, // The Z coordinate of your dashed line
}

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
    GameOver,
}

// ==========================================
// 2. THE SYSTEM (Logic Only)
// ==========================================

/// This system checks if any active sphere has slid past the loss boundary.
/// It only runs when the AppState is `InGame`.
pub fn check_loss_condition(
    // Query: Get the Transform (coordinates) of any Entity that has the `Sphere` component,
    // BUT does NOT have the `InsideLauncher` tag component.
    sphere_query: Query<&Transform, (With<Sphere>, Without<InsideLauncher>)>,
    
    // Access the global configuration resource
    settings: Res<GameSettings>,
    
    // A mechanism to write and change the global game state
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Iterate through every active sliding sphere on the table
    for transform in sphere_query.iter() {
        // Assuming the player is at the positive Z end, looking down toward negative Z.
        // If a ball crosses back past the threshold toward the player, trigger loss.
        if transform.translation.z > settings.loss_boundary_z {
            println!("🚨 A sphere crossed the line at Z: {}! Game Over.", transform.translation.z);
            
            // Tell Bevy to transition to the GameOver state.
            // This will automatically freeze game systems in the next frame.
            next_state.set(AppState::GameOver);
            
            break; // Exit early; we only need one ball to trigger a loss
        }
    }
}
```