use crate::core_math;
use crate::physics_rules::spawn_sphere_entity;
use crate::{AppState, NextSphereId, QueuedInputs, Sphere, SphereId};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ReplayRecord {
    pub seed: u64,
    pub shots: Vec<ShotRecord>,
    pub correction_snapshots: Vec<CorrectionSnapshot>,
}

impl ReplayRecord {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShotRecord {
    pub tick: u64,
    pub x: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CorrectionSnapshot {
    pub tick: u64,
    pub spheres: Vec<SphereSnapshot>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SphereSnapshot {
    pub id: u32,
    pub tier: u8,
    pub x: f32,
    pub z: f32,
    pub vx: f32,
    pub vz: f32,
}

#[derive(Resource, Default, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct GameTick(pub u64);

#[derive(Resource, Default, Debug)]
pub enum ReplayState {
    #[default]
    None,
    Recording {
        record: ReplayRecord,
    },
    Playing {
        record: ReplayRecord,
        next_shot_idx: usize,
        next_snapshot_idx: usize,
        debug_mode: bool,
    },
}

pub struct ReplayPlugin;

impl Plugin for ReplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameTick>()
            .init_resource::<ReplayState>()
            .add_systems(
                FixedUpdate,
                (advance_game_tick, playback_injection_and_correction)
                    .chain()
                    .before(bevy_rapier3d::prelude::PhysicsSet::SyncBackend)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                FixedUpdate,
                record_snapshots_system
                    .after(bevy_rapier3d::prelude::PhysicsSet::Writeback)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn advance_game_tick(mut tick: ResMut<GameTick>) {
    tick.0 += 1;
}

fn playback_injection_and_correction(
    mut commands: Commands,
    tick: Res<GameTick>,
    mut replay_state: ResMut<ReplayState>,
    mut queued_inputs: ResMut<QueuedInputs>,
    mut next_id: ResMut<NextSphereId>,
    // Optional LauncherState from game crate, dynamically updated
    // Note: LauncherState is defined in the game crate, so we cannot access it directly in core
    // without introducing cyclic dependencies. We can instead set queued_inputs target_x,
    // which the launcher system will read and set launcher_state.active_x from.
    mut simulated_spheres: Query<(
        Entity,
        &SphereId,
        &mut Sphere,
        &mut Transform,
        &mut bevy_rapier3d::prelude::Velocity,
    )>,
) {
    if let ReplayState::Playing {
        record,
        next_shot_idx,
        next_snapshot_idx,
        debug_mode,
    } = &mut *replay_state
    {
        // 1. Input Injection
        while *next_shot_idx < record.shots.len() && record.shots[*next_shot_idx].tick == tick.0 {
            let shot = &record.shots[*next_shot_idx];
            queued_inputs.target_x = Some(shot.x);
            queued_inputs.fire_requested = true;
            *next_shot_idx += 1;
        }

        // 2. Drift Correction
        if *next_snapshot_idx < record.correction_snapshots.len()
            && record.correction_snapshots[*next_snapshot_idx].tick == tick.0
        {
            let snapshot = &record.correction_snapshots[*next_snapshot_idx];

            // Build map of current simulated spheres
            let mut sim_map = simulated_spheres
                .iter_mut()
                .map(|(entity, sphere_id, sphere, transform, velocity)| {
                    (sphere_id.0, (entity, sphere, transform, velocity))
                })
                .collect::<std::collections::HashMap<_, _>>();

            let mut matched_sim_ids = std::collections::HashSet::new();

            for recorded in &snapshot.spheres {
                if let Some((_entity, sphere, transform, velocity)) = sim_map.get_mut(&recorded.id)
                {
                    matched_sim_ids.insert(recorded.id);

                    let simulated_pos = transform.translation;
                    let recorded_pos = Vec3::new(recorded.x, transform.translation.y, recorded.z);

                    if *debug_mode {
                        let drift = (simulated_pos - recorded_pos).length();
                        if drift > 0.01 {
                            warn!(
                                "Sphere {} drifted {:.4} units at tick {} (tier {})",
                                recorded.id, drift, tick.0, sphere.tier
                            );
                        }
                    }

                    // Apply corrections
                    sphere.tier = recorded.tier;
                    transform.translation = recorded_pos;
                    velocity.linear = Vec3::new(recorded.vx, 0.0, recorded.vz);
                } else {
                    // Spawn missing sphere
                    let radius = core_math::get_radius(recorded.tier);
                    let pos = Vec3::new(recorded.x, radius, recorded.z);
                    let vel = Vec3::new(recorded.vx, 0.0, recorded.vz);

                    spawn_sphere_entity(&mut commands, recorded.id, recorded.tier, pos, vel);

                    // Update next ID counter to avoid future collisions
                    if recorded.id >= next_id.0 {
                        next_id.0 = recorded.id + 1;
                    }
                }
            }

            for (id, (entity, _, _, _)) in sim_map {
                if !matched_sim_ids.contains(&id) {
                    commands.entity(entity).despawn();
                }
            }

            *next_snapshot_idx += 1;
        }
    }
}

fn record_snapshots_system(
    tick: Res<GameTick>,
    mut replay_state: ResMut<ReplayState>,
    sphere_query: Query<(
        &SphereId,
        &Sphere,
        &Transform,
        &bevy_rapier3d::prelude::Velocity,
    )>,
) {
    if let ReplayState::Recording { record } = &mut *replay_state
        && tick.0 > 0
        && tick.0.is_multiple_of(120)
    {
        let mut spheres = Vec::new();
        for (sphere_id, sphere, transform, velocity) in sphere_query.iter() {
            spheres.push(SphereSnapshot {
                id: sphere_id.0,
                tier: sphere.tier,
                x: transform.translation.x,
                z: transform.translation.z,
                vx: velocity.linear.x,
                vz: velocity.linear.z,
            });
        }
        // Deterministic sort
        spheres.sort_by_key(|s| s.id);
        record.correction_snapshots.push(CorrectionSnapshot {
            tick: tick.0,
            spheres,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics_rules::PhysicsPlugin;
    use crate::{GameSettings, Score};
    use bevy::transform::TransformPlugin;
    use bevy_rapier3d::prelude::RapierPhysicsPlugin;

    #[test]
    fn test_replay_record_serialization() {
        let record = ReplayRecord {
            seed: 12345,
            shots: vec![ShotRecord { tick: 10, x: 1.5 }],
            correction_snapshots: vec![CorrectionSnapshot {
                tick: 120,
                spheres: vec![SphereSnapshot {
                    id: 1,
                    tier: 2,
                    x: 0.5,
                    z: -0.5,
                    vx: 1.0,
                    vz: -1.0,
                }],
            }],
        };

        let json = record.to_json().unwrap();
        let deserialized = ReplayRecord::from_json(&json).unwrap();

        assert_eq!(deserialized.seed, record.seed);
        assert_eq!(deserialized.shots.len(), record.shots.len());
        assert_eq!(deserialized.shots[0].tick, record.shots[0].tick);
        assert_eq!(deserialized.shots[0].x, record.shots[0].x);
        assert_eq!(
            deserialized.correction_snapshots.len(),
            record.correction_snapshots.len()
        );
        assert_eq!(
            deserialized.correction_snapshots[0].tick,
            record.correction_snapshots[0].tick
        );
        assert_eq!(
            deserialized.correction_snapshots[0].spheres.len(),
            record.correction_snapshots[0].spheres.len()
        );
        assert_eq!(
            deserialized.correction_snapshots[0].spheres[0].id,
            record.correction_snapshots[0].spheres[0].id
        );
        assert_eq!(
            deserialized.correction_snapshots[0].spheres[0].tier,
            record.correction_snapshots[0].spheres[0].tier
        );
        assert_eq!(
            deserialized.correction_snapshots[0].spheres[0].x,
            record.correction_snapshots[0].spheres[0].x
        );
        assert_eq!(
            deserialized.correction_snapshots[0].spheres[0].z,
            record.correction_snapshots[0].spheres[0].z
        );
        assert_eq!(
            deserialized.correction_snapshots[0].spheres[0].vx,
            record.correction_snapshots[0].spheres[0].vx
        );
        assert_eq!(
            deserialized.correction_snapshots[0].spheres[0].vz,
            record.correction_snapshots[0].spheres[0].vz
        );
    }

    #[test]
    fn test_replay_playback_injection_and_correction() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.insert_resource(Time::<Fixed>::from_hz(60.0));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ));
        app.add_plugins(TransformPlugin);
        app.add_plugins(RapierPhysicsPlugin::<bevy_rapier3d::prelude::NoUserData>::default());
        app.add_plugins(PhysicsPlugin);
        app.add_plugins(ReplayPlugin);

        app.insert_resource(QueuedInputs::default());
        app.insert_resource(Score::default());
        app.insert_resource(GameSettings::default());

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        let record = ReplayRecord {
            seed: 42,
            shots: vec![ShotRecord { tick: 1, x: 2.5 }],
            correction_snapshots: vec![CorrectionSnapshot {
                tick: 2,
                spheres: vec![
                    SphereSnapshot {
                        id: 1,
                        tier: 1,
                        x: 1.0,
                        z: 2.0,
                        vx: 0.0,
                        vz: 0.0,
                    },
                    SphereSnapshot {
                        id: 2,
                        tier: 2,
                        x: -1.0,
                        z: -2.0,
                        vx: 0.0,
                        vz: 0.0,
                    },
                ],
            }],
        };

        app.insert_resource(ReplayState::Playing {
            record,
            next_shot_idx: 0,
            next_snapshot_idx: 0,
            debug_mode: true,
        });

        // Ticks:
        // Update 1: tick increments to 1. Shot at tick 1 should be injected.
        app.update();

        {
            let queued_inputs = app.world().resource::<QueuedInputs>();
            assert!(queued_inputs.fire_requested);
            assert_eq!(queued_inputs.target_x, Some(2.5));
        }

        // Reset inputs
        app.world_mut()
            .resource_mut::<QueuedInputs>()
            .fire_requested = false;
        app.world_mut().resource_mut::<QueuedInputs>().target_x = None;

        // Before update 2, let's spawn a sphere with ID 1 but with wrong position/velocity to test correction.
        // And spawn another sphere with ID 99 to test despawning unmatched spheres.
        let _sphere_wrong = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1, // ID
            1, // Tier
            Vec3::new(999.0, 0.0, 999.0),
            Vec3::new(999.0, 0.0, 999.0),
        )
        .id();

        let sphere_unmatched = spawn_sphere_entity(
            &mut app.world_mut().commands(),
            99, // ID
            1,  // Tier
            Vec3::ZERO,
            Vec3::ZERO,
        )
        .id();

        app.world_mut().flush(); // flush spawn commands immediately before update

        // At this point tick is 1. The next update will increment tick to 2.
        // The correction should:
        // - Overwrite sphere_wrong (ID 1) to pos (1.0, _, 2.0) and vel (0.5, _, 0.5)
        // - Despawn sphere_unmatched (ID 99)
        // - Spawn a new sphere for ID 2 at pos (-1.0, _, -2.0) and vel (-0.5, _, -0.5)
        app.update();

        // Let's assert state
        assert!(app.world().get_entity(sphere_unmatched).is_err()); // despawned

        let mut query = app.world_mut().query::<(
            &SphereId,
            &Sphere,
            &Transform,
            &bevy_rapier3d::prelude::Velocity,
        )>();
        let mut found_id1 = false;
        let mut found_id2 = false;

        for (sphere_id, sphere, transform, velocity) in query.iter(app.world()) {
            if sphere_id.0 == 1 {
                found_id1 = true;
                assert_eq!(sphere.tier, 1);
                assert!((transform.translation.x - 1.0).abs() < 1e-4);
                assert!((transform.translation.z - 2.0).abs() < 1e-4);
                assert!((velocity.linear.x - 0.0).abs() < 1e-4);
                assert!((velocity.linear.z - 0.0).abs() < 1e-4);
            } else if sphere_id.0 == 2 {
                found_id2 = true;
                assert_eq!(sphere.tier, 2);
                assert!((transform.translation.x - (-1.0)).abs() < 1e-4);
                assert!((transform.translation.z - (-2.0)).abs() < 1e-4);
                assert!((velocity.linear.x - 0.0).abs() < 1e-4);
                assert!((velocity.linear.z - 0.0).abs() < 1e-4);
            }
        }

        assert!(found_id1);
        assert!(found_id2);
    }
}
