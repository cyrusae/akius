use crate::game_state::{Score, Sphere};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use std::collections::HashSet;

#[derive(Component, Debug, Clone)]
pub struct MergeCooldown {
    pub timer: Timer,
}

#[derive(Component)]
pub struct DespawnDelay {
    pub frames: u32,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct MergeEvent {
    pub entity_a: Entity,
    pub entity_b: Entity,
}

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MergeEvent>()
            .add_message::<crate::game_state::MergeBurstEvent>()
            // Merge detection/resolution only makes sense during gameplay; without the
            // state gate these (including an O(n^2) scan) keep running on the menu and
            // end screens.
            .add_systems(
                Update,
                (
                    (detect_collisions, check_distance_merges),
                    resolve_merges,
                    dampen_rebound_velocity,
                    tick_merge_cooldowns,
                    spill_spheres,
                )
                    .chain()
                    .run_if(in_state(crate::game_state::AppState::InGame)),
            )
            // These must keep running outside InGame: pending despawns have to flush
            // even if a merge triggered a state change, and spheres shoved off the
            // table on game over need to be cleaned up once they fall out of view
            // instead of simulating forever beneath the arena.
            .add_systems(
                Update,
                (
                    handle_despawn_delay.after(resolve_merges),
                    despawn_fallen_spheres,
                ),
            )
            .add_systems(
                OnEnter(crate::game_state::AppState::GameOver),
                unlock_all_spheres_on_game_over,
            );
    }
}

/// Helper function to spawn a dynamic sphere with physics and axis-locking constraints
pub fn spawn_sphere_entity<'a>(
    commands: &'a mut Commands,
    tier: u8,
    mut translation: Vec3,
    velocity: Vec3,
) -> bevy::ecs::system::EntityCommands<'a> {
    let radius = crate::core_math::get_radius(tier);
    translation.y = radius;
    // Scale density so smaller spheres are significantly denser and heavier,
    // giving them more relative mass to nudge larger spheres.
    let density = 15.0 / (tier as f32).powf(0.8);
    let cmd = commands.spawn((
        Sphere { tier },
        Transform::from_translation(translation),
        RigidBody::Dynamic,
        Collider::ball(radius),
        ColliderMassProperties::Density(density),
        ActiveEvents::COLLISION_EVENTS,
        Restitution {
            coefficient: 0.05,
            combine_rule: CoefficientCombineRule::Max,
        },
        Friction {
            coefficient: 1.0,
            combine_rule: CoefficientCombineRule::Average,
        },
        Damping {
            linear_damping: 0.8,
            angular_damping: 0.0,
        },
        Velocity {
            linear: velocity,
            angular: Vec3::ZERO,
        },
        LockedAxes::TRANSLATION_LOCKED_Y
            | LockedAxes::ROTATION_LOCKED_X
            | LockedAxes::ROTATION_LOCKED_Y
            | LockedAxes::ROTATION_LOCKED_Z,
    ));
    cmd
}

/// Detects started collisions between two same-tier active spheres and outputs a MergeEvent
pub fn detect_collisions(
    mut collision_events: MessageReader<CollisionEvent>,
    sphere_query: Query<
        &Sphere,
        (
            Without<MergeCooldown>,
            Without<DespawnDelay>,
            Without<crate::game_state::Fulfilling>,
        ),
    >,
    mut merge_events: MessageWriter<MergeEvent>,
) {
    for event in collision_events.read() {
        if let CollisionEvent::Started(e1, e2, _) = *event {
            if let (Ok(s1), Ok(s2)) = (sphere_query.get(e1), sphere_query.get(e2)) {
                if s1.tier == s2.tier {
                    merge_events.write(MergeEvent {
                        entity_a: e1,
                        entity_b: e2,
                    });
                }
            }
        }
    }
}

/// Detects same-tier active spheres that are close enough to touch and merges them.
/// This acts as a robust fallback for resting contacts and visual overlaps.
pub fn check_distance_merges(
    mut merge_events: MessageWriter<MergeEvent>,
    sphere_query: Query<
        (Entity, &Sphere, &Transform),
        (
            Without<MergeCooldown>,
            Without<DespawnDelay>,
            Without<crate::game_state::Fulfilling>,
        ),
    >,
    // Scratch buffers reused across frames: WASM linear memory never shrinks, so
    // avoiding fresh per-frame allocations keeps the heap high-water mark down.
    mut spheres: Local<Vec<(Entity, u8, Vec3)>>,
    mut merged_this_frame: Local<HashSet<Entity>>,
) {
    spheres.clear();
    spheres.extend(
        sphere_query
            .iter()
            .map(|(e, s, t)| (e, s.tier, t.translation)),
    );
    merged_this_frame.clear();

    for i in 0..spheres.len() {
        for j in (i + 1)..spheres.len() {
            let (e1, tier1, pos1) = spheres[i];
            let (e2, tier2, pos2) = spheres[j];

            if tier1 == tier2 {
                let r1 = crate::core_math::get_radius(tier1);
                let r2 = crate::core_math::get_radius(tier2);

                // Merge if distance is within the sum of radii + 0.07 units buffer
                let threshold = r1 + r2 + 0.07;
                let dist = pos1.distance(pos2);

                if dist < threshold
                    && !merged_this_frame.contains(&e1)
                    && !merged_this_frame.contains(&e2)
                {
                    merged_this_frame.insert(e1);
                    merged_this_frame.insert(e2);
                    merge_events.write(MergeEvent {
                        entity_a: e1,
                        entity_b: e2,
                    });
                }
            }
        }
    }
}

/// Resolves scheduled merges, spawning upgraded spheres at midpoints with conserved velocity
pub fn resolve_merges(
    mut commands: Commands,
    mut merge_events: MessageReader<MergeEvent>,
    mut score: ResMut<Score>,
    sphere_query: Query<(&Sphere, &Transform, &Velocity)>,
    mut next_state: ResMut<NextState<crate::game_state::AppState>>,
    mut merge_burst_events: MessageWriter<crate::game_state::MergeBurstEvent>,
) {
    let mut merged_this_frame = HashSet::new();
    for event in merge_events.read() {
        let e1 = event.entity_a;
        let e2 = event.entity_b;

        // Ensure neither entity has already been merged in another pairing this frame
        if merged_this_frame.contains(&e1) || merged_this_frame.contains(&e2) {
            continue;
        }

        // Fetch parent details, ensuring they still exist in the ECS world
        if let (Ok((s1, t1, v1)), Ok((s2, t2, v2))) = (sphere_query.get(e1), sphere_query.get(e2)) {
            if s1.tier == s2.tier {
                let current_tier = s1.tier;
                if current_tier >= 9 {
                    continue; // Maximum tier achieved, cannot merge further
                }

                // Mark parents for next-frame despawn and remove collisions/physics immediately
                merged_this_frame.insert(e1);
                merged_this_frame.insert(e2);
                commands
                    .entity(e1)
                    .insert(DespawnDelay { frames: 1 })
                    .remove::<Collider>()
                    .remove::<RigidBody>()
                    .remove::<ActiveEvents>()
                    .remove::<Velocity>();
                commands
                    .entity(e2)
                    .insert(DespawnDelay { frames: 1 })
                    .remove::<Collider>()
                    .remove::<RigidBody>()
                    .remove::<ActiveEvents>()
                    .remove::<Velocity>();

                // Spawn upgraded sphere at midpoint
                let next_tier = current_tier + 1;
                let midpoint = (t1.translation + t2.translation) * 0.5;

                // 50% combined linear momentum conservation
                let merged_velocity = (v1.linear + v2.linear) * 0.5 * 0.5;

                let mut cmd =
                    spawn_sphere_entity(&mut commands, next_tier, midpoint, merged_velocity);
                cmd.insert(MergeCooldown {
                    timer: Timer::from_seconds(0.35, TimerMode::Once),
                });

                merge_burst_events.write(crate::game_state::MergeBurstEvent {
                    position: midpoint,
                    tier: next_tier,
                });

                // Add points and record peak tier
                score.total += crate::core_math::get_merge_points(next_tier);
                if next_tier > score.peak_tier {
                    score.peak_tier = next_tier;
                }

                // Secret win condition triggered: merging two Tier 8s to form Tier 9
                if next_tier == 9 {
                    info!("Secret Win Condition reached! Transitioning to AppState::Win");
                    next_state.set(crate::game_state::AppState::Win);
                }
            }
        }
    }
}

/// Despawns spheres that have fallen far below the arena (e.g. spilled off the
/// table edge, or shoved off on game over). Without this, unlocked spheres
/// free-fall forever — Rapier never sleeps them, so they cost physics time for
/// as long as the end screen stays open.
pub fn despawn_fallen_spheres(
    mut commands: Commands,
    sphere_query: Query<(Entity, &Transform), With<Sphere>>,
) {
    const DESPAWN_Y: f32 = -20.0;
    for (entity, transform) in sphere_query.iter() {
        if transform.translation.y < DESPAWN_Y {
            commands.entity(entity).despawn();
        }
    }
}

pub fn handle_despawn_delay(mut commands: Commands, mut query: Query<(Entity, &mut DespawnDelay)>) {
    for (entity, mut delay) in query.iter_mut() {
        if delay.frames == 0 {
            commands.entity(entity).despawn();
        } else {
            delay.frames -= 1;
        }
    }
}

pub fn tick_merge_cooldowns(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut MergeCooldown)>,
) {
    for (entity, mut cooldown) in query.iter_mut() {
        cooldown.timer.tick(time.delta());
        if cooldown.timer.is_finished() {
            commands.entity(entity).remove::<MergeCooldown>();
        }
    }
}

/// Dampens positive Z-velocity (upward movement towards the player)
/// using an intensifying gradient that starts at the 1/10-1/8 mark of the board.
pub fn dampen_rebound_velocity(
    mut query: Query<(&Transform, &mut Velocity), With<Sphere>>,
    settings: Res<crate::game_state::GameSettings>,
) {
    let launcher_z = settings.launcher_z;
    let depth = settings.arena_depth;
    let z_start = launcher_z - depth * 0.9; // starts at 10% from the back wall (e.g. -0.6 with default settings)
    let range = depth * 0.8; // intensifies over 80% of the board (e.g. 11.2 with default settings)

    for (transform, mut velocity) in query.iter_mut() {
        if velocity.linear.z > 0.0 && transform.translation.z > z_start {
            let t = ((transform.translation.z - z_start) / range).clamp(0.0, 1.0);
            // Damping factor starts at 1.0 (no damping) and drops to 0.60 (very heavy damping) at the launcher
            let damping = 1.0 - t * 0.40;
            velocity.linear.z *= damping;
        }
    }
}

/// Unlocks the Y-translation and all rotations for spheres that go past Z > settings.launcher_z
/// during gameplay so they visually roll and fall off the edge of the table.
pub fn spill_spheres(
    mut commands: Commands,
    sphere_query: Query<(Entity, &Transform, Option<&LockedAxes>), With<Sphere>>,
    settings: Res<crate::game_state::GameSettings>,
) {
    let launcher_z = settings.launcher_z;
    for (entity, transform, locked_axes) in sphere_query.iter() {
        if transform.translation.z > launcher_z {
            if let Some(axes) = locked_axes {
                if *axes != LockedAxes::empty() {
                    // Setting LockedAxes::empty() ensures Rapier registers the change and unlocks the axes
                    commands.entity(entity).insert(LockedAxes::empty());
                }
            }
        }
    }
}

/// Unlocks the Y-translation and all rotations for all active spheres on the board
/// and applies a positive Z-velocity push when the game transitions to AppState::GameOver.
pub fn unlock_all_spheres_on_game_over(
    mut commands: Commands,
    mut sphere_query: Query<(Entity, &mut Velocity), With<Sphere>>,
) {
    for (entity, mut velocity) in sphere_query.iter_mut() {
        commands.entity(entity).insert(LockedAxes::empty());
        velocity.linear.z = 2.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::AppState;
    use bevy_rapier3d::rapier::geometry::CollisionEventFlags;

    #[test]
    fn test_sphere_spawn_physics_attributes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let entity = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::new(1.0, 0.0, 2.0),
            Vec3::new(0.0, 0.0, -10.0),
        )
        .id();

        app.update();

        // Verify custom components and Bevy Rapier components exist and match
        let world = app.world();
        let sphere = world
            .entity(entity)
            .get::<Sphere>()
            .expect("Missing Sphere component");
        assert_eq!(sphere.tier, 1);

        let rigid_body = world
            .entity(entity)
            .get::<RigidBody>()
            .expect("Missing RigidBody");
        assert_eq!(*rigid_body, RigidBody::Dynamic);

        let locked_axes = world
            .entity(entity)
            .get::<LockedAxes>()
            .expect("Missing LockedAxes");
        assert!(locked_axes.contains(LockedAxes::TRANSLATION_LOCKED_Y));
        assert!(locked_axes.contains(LockedAxes::ROTATION_LOCKED));

        let restitution = world
            .entity(entity)
            .get::<Restitution>()
            .expect("Missing Restitution");
        assert_eq!(restitution.coefficient, 0.05);

        let friction = world
            .entity(entity)
            .get::<Friction>()
            .expect("Missing Friction");
        assert_eq!(friction.coefficient, 1.0);

        let velocity = world
            .entity(entity)
            .get::<Velocity>()
            .expect("Missing Velocity");
        assert_eq!(velocity.linear, Vec3::new(0.0, 0.0, -10.0));
    }

    #[test]
    fn test_collision_midpoint_merge() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.add_message::<CollisionEvent>();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });

        app.insert_resource(crate::game_state::GameSettings::default());

        // Merge systems are gated on AppState::InGame
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        // Spawn two adjacent Tier 1 spheres
        let entity_a = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        )
        .id();
        let entity_b = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(-10.0, 0.0, 0.0),
        )
        .id();

        app.update(); // Apply spawn commands

        // Manually send start collision event
        app.world_mut()
            .resource_mut::<Messages<CollisionEvent>>()
            .write(CollisionEvent::Started(
                entity_a,
                entity_b,
                CollisionEventFlags::empty(),
            ));

        app.update(); // Tick 1: detects collision, generates MergeEvent, resolves it (spawns Tier 2 commands)
        app.update(); // Tick 2: applies spawn command for Tier 2

        // Verify parents are despawned
        assert!(app.world().get_entity(entity_a).is_err());
        assert!(app.world().get_entity(entity_b).is_err());

        // Locate new Tier 2 sphere
        let mut query = app
            .world_mut()
            .query::<(Entity, &Sphere, &Transform, &Velocity)>();
        let mut found = false;

        for (_ent, sphere, transform, velocity) in query.iter(app.world()) {
            if sphere.tier == 2 {
                assert_eq!(
                    transform.translation,
                    Vec3::new(1.0, crate::core_math::get_radius(2), 0.0)
                ); // Midpoint of (0,0,0) and (2,0,0)
                assert_eq!(velocity.linear, Vec3::new(0.0, 0.0, 0.0)); // 50% of (10 - 10)
                found = true;
            }
        }
        assert!(found, "New Tier 2 sphere not found in world");

        // Verify score update (resulting Tier 2 merge = 200 points)
        let score = app.world().resource::<Score>();
        assert_eq!(score.total, 200);
        assert_eq!(score.peak_tier, 2);
    }

    #[test]
    fn test_double_merge_safety() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.add_message::<CollisionEvent>();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });

        app.insert_resource(crate::game_state::GameSettings::default());

        // Merge systems are gated on AppState::InGame
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        let entity_a = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::ZERO,
        )
        .id();
        let entity_b = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
        )
        .id();
        let entity_c = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::ZERO,
        )
        .id();

        app.update();

        // Inject dual collisions (A touches B, B touches C)
        app.world_mut()
            .resource_mut::<Messages<CollisionEvent>>()
            .write(CollisionEvent::Started(
                entity_a,
                entity_b,
                CollisionEventFlags::empty(),
            ));
        app.world_mut()
            .resource_mut::<Messages<CollisionEvent>>()
            .write(CollisionEvent::Started(
                entity_b,
                entity_c,
                CollisionEventFlags::empty(),
            ));

        app.update(); // Resolve
        app.update(); // Apply

        // A and B should be merged (despawned)
        assert!(app.world().get_entity(entity_a).is_err());
        assert!(app.world().get_entity(entity_b).is_err());

        // C should remain unchanged in the world
        assert!(app.world().get_entity(entity_c).is_ok());
    }

    #[test]
    fn test_merge_momentum_conservation() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.add_message::<CollisionEvent>();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });

        app.insert_resource(crate::game_state::GameSettings::default());

        // Merge systems are gated on AppState::InGame
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        let entity_a = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .id();
        let entity_b = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::ZERO,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .id();

        app.update();

        app.world_mut()
            .resource_mut::<Messages<CollisionEvent>>()
            .write(CollisionEvent::Started(
                entity_a,
                entity_b,
                CollisionEventFlags::empty(),
            ));

        app.update();
        app.update();

        let mut query = app.world_mut().query::<(&Sphere, &Velocity)>();
        let mut checked = false;

        for (sphere, velocity) in query.iter(app.world()) {
            if sphere.tier == 2 {
                // Expected momentum = 0.5 * (10 + 20) * 0.5 = 7.5
                assert_eq!(velocity.linear, Vec3::new(7.5, 0.0, 0.0));
                checked = true;
            }
        }
        assert!(checked);
    }

    #[test]
    fn test_spill_spheres() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(crate::game_state::GameSettings {
            launcher_z: 12.0,
            ..default()
        });
        app.add_systems(Update, spill_spheres);

        // Spawn a sphere with Z < 12 (launcher_z), translation.z = 10.0
        let entity_inside = app
            .world_mut()
            .spawn((
                Sphere { tier: 1 },
                Transform::from_xyz(0.0, 0.0, 10.0),
                LockedAxes::TRANSLATION_LOCKED_Y,
            ))
            .id();

        // Spawn a sphere with Z > 12, translation.z = 13.0
        let entity_spilled = app
            .world_mut()
            .spawn((
                Sphere { tier: 1 },
                Transform::from_xyz(0.0, 0.0, 13.0),
                LockedAxes::TRANSLATION_LOCKED_Y,
            ))
            .id();

        app.update();

        // Verify entity_inside still has its axes locked
        let axes_inside = app
            .world()
            .entity(entity_inside)
            .get::<LockedAxes>()
            .unwrap();
        assert_eq!(*axes_inside, LockedAxes::TRANSLATION_LOCKED_Y);

        // Verify entity_spilled has LockedAxes::empty()
        let axes_spilled = app
            .world()
            .entity(entity_spilled)
            .get::<LockedAxes>()
            .unwrap();
        assert_eq!(*axes_spilled, LockedAxes::empty());
    }

    #[test]
    fn test_entity_count_soak() {
        use crate::game_state::{ActiveFulfillment, Fulfilling};
        use crate::visuals::{
            cleanup_orphaned_labels, handle_placeholder_bursts, on_sphere_added,
            update_matrix_particles, MatrixParticleAssets, TierMaterials, TierMeshes,
        };

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            bevy::asset::AssetPlugin::default(),
        ));

        // Add observer
        app.add_observer(on_sphere_added);

        // Add message types
        app.add_message::<CollisionEvent>();
        app.add_message::<MergeEvent>();
        app.add_message::<crate::game_state::MergeBurstEvent>();
        app.add_message::<crate::game_state::FulfillmentBurstEvent>();

        // Insert required resources
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });
        app.insert_resource(crate::game_state::ActiveOrder { target_tier: 6 });
        app.insert_resource(ActiveFulfillment::default());
        app.insert_resource(crate::game_state::GameSettings {
            launcher_z: 12.0,
            ..default()
        });
        app.insert_resource(crate::game_state::ColorblindMode(true));

        // Insert dummy assets for observer and particle systems
        app.insert_resource(TierMaterials {
            normal: std::array::from_fn(|_| Handle::default()),
            core: std::array::from_fn(|_| Handle::default()),
        });
        app.insert_resource(TierMeshes {
            outer: std::array::from_fn(|_| Handle::default()),
            core: std::array::from_fn(|_| Handle::default()),
        });
        app.insert_resource(MatrixParticleAssets {
            mesh: Handle::default(),
            merge_material: Handle::default(),
            fulfill_material: Handle::default(),
        });

        // Add systems
        app.add_plugins(PhysicsPlugin);
        app.add_systems(
            Update,
            (
                cleanup_orphaned_labels,
                handle_placeholder_bursts,
                update_matrix_particles,
                crate::game_state::check_order_fulfillment,
            )
                .run_if(in_state(AppState::InGame)),
        );

        // Set time update strategy to manual
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ));

        app.init_state::<AppState>();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update(); // Apply state transition

        // Record baseline entities (e.g. camera, window, static objects, etc.)
        let baseline_count = app.world_mut().query::<Entity>().iter(app.world()).count();

        // 1. Spawn two tier-1 spheres
        let entity_a = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::new(-0.2, 0.0, 0.0),
            Vec3::ZERO,
        )
        .id();
        let entity_b = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::new(0.2, 0.0, 0.0),
            Vec3::ZERO,
        )
        .id();

        app.update(); // Spawn them, runs observer

        // Trigger collision between them
        app.world_mut()
            .resource_mut::<Messages<CollisionEvent>>()
            .write(CollisionEvent::Started(
                entity_a,
                entity_b,
                CollisionEventFlags::empty(),
            ));

        // Advance 30 frames to let merge resolve, upgraded sphere spawn, particles burst, and merge cooldown to finish (takes 24 frames/0.4s)
        for _ in 0..30 {
            app.update();
        }

        // Verify upgraded sphere is spawned (tier 2)
        let mut query = app.world_mut().query::<(Entity, &Sphere)>();
        let mut found_tier2 = None;
        for (entity, sphere) in query.iter(app.world()) {
            if sphere.tier == 2 {
                found_tier2 = Some(entity);
            }
        }
        assert!(found_tier2.is_some(), "Tier 2 sphere should have spawned");
        let tier2_entity = found_tier2.unwrap();

        // Change target order to tier 2 to trigger order fulfillment on it
        app.world_mut()
            .resource_mut::<crate::game_state::ActiveOrder>()
            .target_tier = 2;

        app.update(); // check_order_fulfillment runs and sets target to fulfilling

        // Verify it is now Fulfilling
        assert!(
            app.world().entity(tier2_entity).contains::<Fulfilling>(),
            "Sphere should be in Fulfilling state"
        );

        // Run app for 100 frames to let fulfillment timer (1.2s -> 72 frames) complete
        // and despawn the fulfilling sphere
        for _ in 0..100 {
            app.update();
        }

        // Verify the sphere is despawned
        assert!(
            app.world().get_entity(tier2_entity).is_err(),
            "Fulfilling sphere should be despawned"
        );

        // Run another 150 frames to ensure all particles (max 2.0s -> 120 frames) have fully expired and despawned,
        // and orphaned labels are cleaned up
        for _ in 0..150 {
            app.update();
        }

        // Verify total entity count is back to baseline
        let final_count = app.world_mut().query::<Entity>().iter(app.world()).count();

        assert_eq!(
            final_count, baseline_count,
            "Leaked entities found! Baseline: {}, Final: {}",
            baseline_count, final_count
        );
    }

    #[test]
    fn test_dampen_rebound_velocity() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(crate::game_state::GameSettings {
            launcher_z: 12.0,
            arena_depth: 14.0,
            ..default()
        });
        app.add_systems(Update, dampen_rebound_velocity);

        // Spawn a sphere moving towards the player (+Z) from Z = 5.0 (well within the damping zone)
        let entity_damping = app
            .world_mut()
            .spawn((
                Sphere { tier: 1 },
                Transform::from_xyz(0.0, 0.0, 5.0),
                Velocity {
                    linear: Vec3::new(0.0, 0.0, 10.0),
                    angular: Vec3::ZERO,
                },
            ))
            .id();

        // Spawn a sphere moving away from the player (-Z) from Z = 5.0
        let entity_no_damping_dir = app
            .world_mut()
            .spawn((
                Sphere { tier: 1 },
                Transform::from_xyz(0.0, 0.0, 5.0),
                Velocity {
                    linear: Vec3::new(0.0, 0.0, -10.0),
                    angular: Vec3::ZERO,
                },
            ))
            .id();

        // Spawn a sphere moving towards the player (+Z) from Z = -1.5 (behind the damping start Z_start = -0.6)
        let entity_no_damping_pos = app
            .world_mut()
            .spawn((
                Sphere { tier: 1 },
                Transform::from_xyz(0.0, 0.0, -1.5),
                Velocity {
                    linear: Vec3::new(0.0, 0.0, 10.0),
                    angular: Vec3::ZERO,
                },
            ))
            .id();

        app.update();

        // Verify entity_damping has reduced positive Z velocity
        let vel_damping = app
            .world()
            .entity(entity_damping)
            .get::<Velocity>()
            .unwrap();
        assert!(vel_damping.linear.z < 10.0);
        assert!(vel_damping.linear.z > 0.0);

        // Verify entity_no_damping_dir has unchanged negative Z velocity
        let vel_no_damping_dir = app
            .world()
            .entity(entity_no_damping_dir)
            .get::<Velocity>()
            .unwrap();
        assert_eq!(vel_no_damping_dir.linear.z, -10.0);

        // Verify entity_no_damping_pos has unchanged positive Z velocity because it is before the damping zone starts
        let vel_no_damping_pos = app
            .world()
            .entity(entity_no_damping_pos)
            .get::<Velocity>()
            .unwrap();
        assert_eq!(vel_no_damping_pos.linear.z, 10.0);
    }

}


