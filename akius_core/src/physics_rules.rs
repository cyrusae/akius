use super::Sphere;
use super::*;
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
            .add_message::<MergeBurstEvent>()
            .init_resource::<NextSphereId>()
            .init_resource::<GameRng>()
            .add_systems(
                FixedUpdate,
                (
                    (detect_collisions, check_distance_merges),
                    resolve_merges,
                    dampen_rebound_velocity,
                    tick_merge_cooldowns,
                    spill_spheres,
                )
                    .chain()
                    .after(PhysicsSet::Writeback)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                FixedUpdate,
                (
                    handle_despawn_delay.after(resolve_merges),
                    despawn_fallen_spheres,
                )
                    .after(PhysicsSet::Writeback),
            )
            .add_systems(OnEnter(AppState::GameOver), unlock_all_spheres_on_game_over);
    }
}

pub fn spawn_sphere_entity<'a>(
    commands: &'a mut Commands,
    id: u32,
    tier: u8,
    mut translation: Vec3,
    velocity: Vec3,
) -> bevy::ecs::system::EntityCommands<'a> {
    let radius = core_math::get_radius(tier);
    translation.y = radius;
    let density = 15.0 / (tier as f32).powf(0.8);
    commands.spawn((
        Sphere { tier },
        SphereId(id),
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
    ))
}

pub fn detect_collisions(
    mut collision_events: MessageReader<CollisionEvent>,
    sphere_query: Query<
        &Sphere,
        (
            Without<MergeCooldown>,
            Without<DespawnDelay>,
            Without<Fulfilling>,
        ),
    >,
    mut merge_events: MessageWriter<MergeEvent>,
) {
    for event in collision_events.read() {
        if let CollisionEvent::Started(e1, e2, _) = *event
            && let (Ok(s1), Ok(s2)) = (sphere_query.get(e1), sphere_query.get(e2))
            && s1.tier == s2.tier
        {
            merge_events.write(MergeEvent {
                entity_a: e1,
                entity_b: e2,
            });
        }
    }
}

pub fn check_distance_merges(
    mut merge_events: MessageWriter<MergeEvent>,
    sphere_query: Query<
        (Entity, &Sphere, &Transform),
        (
            Without<MergeCooldown>,
            Without<DespawnDelay>,
            Without<Fulfilling>,
        ),
    >,
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
                let r1 = core_math::get_radius(tier1);
                let r2 = core_math::get_radius(tier2);

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

pub fn resolve_merges(
    mut commands: Commands,
    mut merge_events: MessageReader<MergeEvent>,
    mut score: ResMut<Score>,
    sphere_query: Query<(&Sphere, &Transform, &Velocity)>,
    mut next_state: ResMut<NextState<AppState>>,
    mut merge_burst_events: MessageWriter<MergeBurstEvent>,
    mut next_id: ResMut<NextSphereId>,
) {
    let mut merged_this_frame = HashSet::new();
    for event in merge_events.read() {
        let e1 = event.entity_a;
        let e2 = event.entity_b;

        if merged_this_frame.contains(&e1) || merged_this_frame.contains(&e2) {
            continue;
        }

        if let (Ok((s1, t1, v1)), Ok((s2, t2, v2))) = (sphere_query.get(e1), sphere_query.get(e2))
            && s1.tier == s2.tier
        {
            let current_tier = s1.tier;
            if current_tier >= 9 {
                continue;
            }

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

            let next_tier = current_tier + 1;
            let midpoint = (t1.translation + t2.translation) * 0.5;

            let merged_velocity = (v1.linear + v2.linear) * 0.5 * 0.5;

            let id = next_id.0;
            next_id.0 += 1;
            let mut cmd =
                spawn_sphere_entity(&mut commands, id, next_tier, midpoint, merged_velocity);
            cmd.insert(MergeCooldown {
                timer: Timer::from_seconds(0.35, TimerMode::Once),
            });

            merge_burst_events.write(MergeBurstEvent {
                position: midpoint,
                tier: next_tier,
            });

            score.total += core_math::get_merge_points(next_tier);
            if next_tier > score.peak_tier {
                score.peak_tier = next_tier;
            }

            if next_tier == 9 {
                info!("Secret Win Condition reached! Transitioning to AppState::Win");
                next_state.set(AppState::Win);
            }
        }
    }
}

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

pub fn dampen_rebound_velocity(
    mut query: Query<(&Transform, &mut Velocity), With<Sphere>>,
    settings: Res<GameSettings>,
) {
    let launcher_z = settings.launcher_z;
    let depth = settings.arena_depth;
    let z_start = launcher_z - depth * 0.9;
    let range = depth * 0.8;

    for (transform, mut velocity) in query.iter_mut() {
        if velocity.linear.z > 0.0 && transform.translation.z > z_start {
            let t = ((transform.translation.z - z_start) / range).clamp(0.0, 1.0);
            let damping = 1.0 - t * 0.40;
            velocity.linear.z *= damping;
        }
    }
}

pub fn spill_spheres(
    mut commands: Commands,
    sphere_query: Query<(Entity, &Transform, Option<&LockedAxes>), With<Sphere>>,
    settings: Res<GameSettings>,
) {
    let launcher_z = settings.launcher_z;
    for (entity, transform, locked_axes) in sphere_query.iter() {
        if transform.translation.z > launcher_z
            && let Some(axes) = locked_axes
            && *axes != LockedAxes::empty()
        {
            commands.entity(entity).insert(LockedAxes::empty());
        }
    }
}

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
            1, // ID
            1, // Tier
            Vec3::new(1.0, 0.0, 2.0),
            Vec3::new(0.0, 0.0, -10.0),
        )
        .id();

        app.update();

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
        app.insert_resource(Time::<Fixed>::from_hz(60.0));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ));
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.add_message::<CollisionEvent>();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });

        app.insert_resource(GameSettings::default());

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        let entity_a = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1, // ID
            1, // Tier
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        )
        .id();
        let entity_b = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            2, // ID
            1, // Tier
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(-10.0, 0.0, 0.0),
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

        assert!(app.world().get_entity(entity_a).is_err());
        assert!(app.world().get_entity(entity_b).is_err());

        let mut query = app
            .world_mut()
            .query::<(Entity, &Sphere, &Transform, &Velocity)>();
        let mut found = false;

        for (_ent, sphere, transform, velocity) in query.iter(app.world()) {
            if sphere.tier == 2 {
                assert_eq!(
                    transform.translation,
                    Vec3::new(1.0, core_math::get_radius(2), 0.0)
                );
                assert_eq!(velocity.linear, Vec3::new(0.0, 0.0, 0.0));
                found = true;
            }
        }
        assert!(found, "New Tier 2 sphere not found in world");

        let score = app.world().resource::<Score>();
        assert_eq!(score.total, 200);
        assert_eq!(score.peak_tier, 2);
    }

    #[test]
    fn test_double_merge_safety() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Time::<Fixed>::from_hz(60.0));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ));
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.add_message::<CollisionEvent>();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });

        app.insert_resource(GameSettings::default());

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        let entity_a = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1, // ID
            1, // Tier
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::ZERO,
        )
        .id();
        let entity_b = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            2, // ID
            1, // Tier
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
        )
        .id();
        let entity_c = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            3, // ID
            1, // Tier
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::ZERO,
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
        app.world_mut()
            .resource_mut::<Messages<CollisionEvent>>()
            .write(CollisionEvent::Started(
                entity_b,
                entity_c,
                CollisionEventFlags::empty(),
            ));

        app.update();
        app.update();

        assert!(app.world().get_entity(entity_a).is_err());
        assert!(app.world().get_entity(entity_b).is_err());
        assert!(app.world().get_entity(entity_c).is_ok());
    }

    #[test]
    fn test_merge_momentum_conservation() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Time::<Fixed>::from_hz(60.0));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ));
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.add_message::<CollisionEvent>();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });

        app.insert_resource(GameSettings::default());

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        let entity_a = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1, // ID
            1, // Tier
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .id();
        let entity_b = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            2, // ID
            1, // Tier
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
        app.insert_resource(GameSettings {
            launcher_z: 12.0,
            ..default()
        });
        app.add_systems(Update, spill_spheres);

        let entity_inside = app
            .world_mut()
            .spawn((
                Sphere { tier: 1 },
                Transform::from_xyz(0.0, 0.0, 12.0), // Exactly on boundary, should NOT spill
                LockedAxes::TRANSLATION_LOCKED_Y,
            ))
            .id();

        let entity_spilled = app
            .world_mut()
            .spawn((
                Sphere { tier: 1 },
                Transform::from_xyz(0.0, 0.0, 12.0001), // Just past boundary, should spill
                LockedAxes::TRANSLATION_LOCKED_Y,
            ))
            .id();

        app.update();

        let axes_inside = app
            .world()
            .entity(entity_inside)
            .get::<LockedAxes>()
            .unwrap();
        assert_eq!(*axes_inside, LockedAxes::TRANSLATION_LOCKED_Y);

        let axes_spilled = app
            .world()
            .entity(entity_spilled)
            .get::<LockedAxes>()
            .unwrap();
        assert_eq!(*axes_spilled, LockedAxes::empty());
    }

    #[test]
    fn test_dampen_rebound_velocity() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(GameSettings {
            launcher_z: 12.0,
            arena_depth: 14.0,
            ..default()
        });
        app.add_systems(Update, dampen_rebound_velocity);

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

        let entity_no_damping_pos = app
            .world_mut()
            .spawn((
                Sphere { tier: 1 },
                Transform::from_xyz(0.0, 0.0, -1.5), // z_start is -0.6, so -1.5 is outside range
                Velocity {
                    linear: Vec3::new(0.0, 0.0, 10.0),
                    angular: Vec3::ZERO,
                },
            ))
            .id();

        app.update();

        let vel_damping = app
            .world()
            .entity(entity_damping)
            .get::<Velocity>()
            .unwrap();
        // Exact damped velocity: 10.0 * (1.0 - 0.5 * 0.40) = 8.0
        assert_eq!(vel_damping.linear.z, 8.0);

        let vel_no_damping_dir = app
            .world()
            .entity(entity_no_damping_dir)
            .get::<Velocity>()
            .unwrap();
        assert_eq!(vel_no_damping_dir.linear.z, -10.0);

        let vel_no_damping_pos = app
            .world()
            .entity(entity_no_damping_pos)
            .get::<Velocity>()
            .unwrap();
        assert_eq!(vel_no_damping_pos.linear.z, 10.0);
    }

    #[test]
    fn test_distance_merge_boundaries() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        // Force manual FixedUpdate execution
        app.insert_resource(Time::<Fixed>::from_hz(60.0));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ));
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.add_message::<CollisionEvent>();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(Score::default());
        app.insert_resource(GameSettings::default());

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        // 1. Spawning two spheres just out of range:
        // Radius(1) = 0.5. sum of radii + 0.07 = 1.07
        let entity_a = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1, // ID
            1, // Tier
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::ZERO,
        )
        .id();
        let entity_b = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            2,                         // ID
            1,                         // Tier
            Vec3::new(1.07, 0.0, 0.0), // exactly 1.07 distance
            Vec3::ZERO,
        )
        .id();

        app.update();

        // They should NOT merge
        assert!(app.world().get_entity(entity_a).is_ok());
        assert!(app.world().get_entity(entity_b).is_ok());

        // Despawn them
        app.world_mut().entity_mut(entity_a).despawn();
        app.world_mut().entity_mut(entity_b).despawn();

        // 2. Spawning two spheres just in range:
        let entity_c = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1, // ID
            1, // Tier
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::ZERO,
        )
        .id();
        let entity_d = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            2,                          // ID
            1,                          // Tier
            Vec3::new(1.069, 0.0, 0.0), // slightly less than 1.07
            Vec3::ZERO,
        )
        .id();

        app.update(); // run detect/resolve
        app.update(); // handle despawn delay

        // They SHOULD merge
        assert!(app.world().get_entity(entity_c).is_err());
        assert!(app.world().get_entity(entity_d).is_err());
    }
}
