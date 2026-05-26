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
            .add_systems(
                Update,
                (
                    (detect_collisions, check_distance_merges),
                    resolve_merges,
                    tick_merge_cooldowns,
                    handle_despawn_delay,
                    spill_spheres,
                )
                    .chain(),
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
        Visibility::default(),
    ));
    cmd
}

/// Detects started collisions between two same-tier active spheres and outputs a MergeEvent
pub fn detect_collisions(
    mut collision_events: MessageReader<CollisionEvent>,
    sphere_query: Query<
        &Sphere,
        (
            Without<crate::game_state::InsideLauncher>,
            Without<MergeCooldown>,
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
            Without<crate::game_state::InsideLauncher>,
            Without<MergeCooldown>,
            Without<crate::game_state::Fulfilling>,
        ),
    >,
) {
    let spheres: Vec<_> = sphere_query.iter().collect();
    let mut merged_this_frame = HashSet::new();

    for i in 0..spheres.len() {
        for j in (i + 1)..spheres.len() {
            let (e1, s1, t1) = spheres[i];
            let (e2, s2, t2) = spheres[j];

            if s1.tier == s2.tier {
                let r1 = crate::core_math::get_radius(s1.tier);
                let r2 = crate::core_math::get_radius(s2.tier);

                // Merge if distance is within the sum of radii + 0.07 units buffer
                let threshold = r1 + r2 + 0.07;
                let dist = t1.translation.distance(t2.translation);

                if dist < threshold {
                    if !merged_this_frame.contains(&e1) && !merged_this_frame.contains(&e2) {
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
}

/// Resolves scheduled merges, spawning upgraded spheres at midpoints with conserved velocity
pub fn resolve_merges(
    mut commands: Commands,
    mut merge_events: MessageReader<MergeEvent>,
    mut score: ResMut<Score>,
    sphere_query: Query<(&Sphere, &Transform, &Velocity)>,
    mut next_state: Option<ResMut<NextState<crate::game_state::AppState>>>,
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

                // Add points and record peak tier
                score.total += crate::core_math::get_merge_points(next_tier);
                if next_tier > score.peak_tier {
                    score.peak_tier = next_tier;
                }

                // Secret win condition triggered: merging two Tier 8s to form Tier 9
                if next_tier == 9 {
                    info!("Secret Win Condition reached! Transitioning to AppState::Win");
                    if let Some(ref mut ns) = next_state {
                        ns.set(crate::game_state::AppState::Win);
                    }
                }
            }
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
/// Unlocks the Y-translation and all rotations for spheres that go past Z > settings.launcher_z
/// during gameplay so they visually roll and fall off the edge of the table.
pub fn spill_spheres(
    mut commands: Commands,
    sphere_query: Query<(Entity, &Transform, Option<&LockedAxes>), With<Sphere>>,
    settings: Option<Res<crate::game_state::GameSettings>>,
) {
    let launcher_z = settings.map(|s| s.launcher_z).unwrap_or(12.0);
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
        app.add_message::<CollisionEvent>();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });

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
                assert_eq!(transform.translation, Vec3::new(1.0, crate::core_math::get_radius(2), 0.0)); // Midpoint of (0,0,0) and (2,0,0)
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
        app.add_message::<CollisionEvent>();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });

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
        app.add_message::<CollisionEvent>();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });

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
}
