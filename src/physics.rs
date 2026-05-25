use crate::game_state::{Score, Sphere};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use std::collections::HashSet;

#[derive(Message, Debug, Clone, Copy)]
pub struct MergeEvent {
    pub entity_a: Entity,
    pub entity_b: Entity,
}

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MergeEvent>()
            .add_systems(Update, (detect_collisions, resolve_merges).chain());
    }
}

/// Helper function to spawn a dynamic sphere with physics and axis-locking constraints
pub fn spawn_sphere_entity<'a>(
    commands: &'a mut Commands,
    tier: u8,
    translation: Vec3,
    velocity: Vec3,
) -> bevy::ecs::system::EntityCommands<'a> {
    let radius = crate::core_math::get_radius(tier);
    let cmd = commands.spawn((
        Sphere { tier },
        Transform::from_translation(translation),
        RigidBody::Dynamic,
        Collider::ball(radius),
        Restitution {
            coefficient: 0.3,
            combine_rule: CoefficientCombineRule::Min,
        },
        Friction {
            coefficient: 0.5,
            combine_rule: CoefficientCombineRule::Average,
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
    sphere_query: Query<&Sphere, Without<crate::game_state::InsideLauncher>>,
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

/// Resolves scheduled merges, spawning upgraded spheres at midpoints with conserved velocity
pub fn resolve_merges(
    mut commands: Commands,
    mut merge_events: MessageReader<MergeEvent>,
    mut score: ResMut<Score>,
    sphere_query: Query<(&Sphere, &Transform, &Velocity)>,
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
                if current_tier >= 13 {
                    continue; // Maximum tier achieved, cannot merge further
                }

                // Register entities as merged to block duplicate pairings
                merged_this_frame.insert(e1);
                merged_this_frame.insert(e2);

                // Despawn parents
                commands.entity(e1).despawn();
                commands.entity(e2).despawn();

                // Spawn upgraded sphere at midpoint
                let next_tier = current_tier + 1;
                let midpoint = (t1.translation + t2.translation) * 0.5;

                // 50% combined linear momentum conservation
                let merged_velocity = (v1.linear + v2.linear) * 0.5 * 0.5;

                spawn_sphere_entity(&mut commands, next_tier, midpoint, merged_velocity);

                // Add points and record peak tier
                score.total += crate::core_math::get_merge_points(next_tier);
                if next_tier > score.peak_tier {
                    score.peak_tier = next_tier;
                }
            }
        }
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
        assert_eq!(restitution.coefficient, 0.3);

        let friction = world
            .entity(entity)
            .get::<Friction>()
            .expect("Missing Friction");
        assert_eq!(friction.coefficient, 0.5);

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
                assert_eq!(transform.translation, Vec3::new(1.0, 0.0, 0.0)); // Midpoint of (0,0,0) and (2,0,0)
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
