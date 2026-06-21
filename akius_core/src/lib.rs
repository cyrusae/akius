use bevy::prelude::*;
use bevy_rapier3d::prelude::{ActiveEvents, RigidBody, Velocity};
use rand::rngs::StdRng;
use rand::SeedableRng;

pub mod core_math;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
    GameOver,
    Win,
}

#[derive(Component, Debug, Clone)]
pub struct Sphere {
    pub tier: u8,
}

#[derive(Component)]
pub struct LossTracker {
    pub timer: Timer,
}

#[derive(Component)]
pub struct Fulfilling;

#[derive(Resource, Clone)]
pub struct GameSettings {
    pub launcher_z: f32,
    pub arena_width: f32,
    pub arena_depth: f32,
    pub wall_height: f32,
    pub launch_speed: f32,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            launcher_z: 12.0,
            arena_width: 8.0,
            arena_depth: 14.0,
            wall_height: 1.5,
            launch_speed: 15.0,
        }
    }
}

/// Whether the colorblind-friendly sphere pattern overlay is active.
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct ColorblindMode(pub bool);

impl Default for ColorblindMode {
    fn default() -> Self {
        Self(true)
    }
}

/// Whether the aiming guide line is active.
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct AimLineMode(pub bool);

impl Default for AimLineMode {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Default, Clone)]
pub struct Score {
    pub total: u32,
    pub peak_tier: u8,
    pub completed_orders: u32,
}

#[derive(Resource, Clone)]
pub struct ActiveOrder {
    pub target_tier: u8,
}

#[derive(Resource, Clone)]
pub struct DispenserQueue {
    pub current: u8,
    pub next: u8,
}

#[derive(Resource, Clone)]
pub struct ActiveFulfillment {
    pub entity: Option<Entity>,
    pub timer: Timer,
}

impl Default for ActiveFulfillment {
    fn default() -> Self {
        Self {
            entity: None,
            timer: Timer::from_seconds(1.2, TimerMode::Once),
        }
    }
}

#[derive(Resource, Default, Debug, Clone)]
pub struct QueuedInputs {
    pub fire_requested: bool,
    pub target_x: Option<f32>,
}

#[derive(Resource)]
pub struct GameRng {
    pub seed: u64,
    pub rng: StdRng,
}

impl Default for GameRng {
    fn default() -> Self {
        let seed = rand::random::<u64>();
        Self {
            seed,
            rng: StdRng::seed_from_u64(seed),
        }
    }
}

impl GameRng {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            rng: StdRng::seed_from_u64(seed),
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SphereId(pub u32);

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct NextSphereId(pub u32);

#[derive(Message, Debug, Clone, Copy)]
pub struct MergeBurstEvent {
    pub position: Vec3,
    pub tier: u8,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct FulfillmentBurstEvent {
    pub position: Vec3,
    pub tier: u8,
}

pub fn check_loss_condition(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sphere_query: Query<
        (Entity, &Transform, Option<&mut LossTracker>),
        (With<Sphere>, Without<Fulfilling>),
    >,
) {
    for (entity, transform, loss_tracker) in sphere_query.iter_mut() {
        let is_past_z = transform.translation.z > settings.launcher_z + 0.2;
        let is_fallen_y = transform.translation.y < -0.2;
        if is_past_z || is_fallen_y {
            if let Some(mut tracker) = loss_tracker {
                tracker.timer.tick(time.delta());
                if tracker.timer.is_finished() {
                    next_state.set(AppState::GameOver);
                }
            } else {
                commands.entity(entity).insert(LossTracker {
                    timer: Timer::from_seconds(0.5, TimerMode::Once),
                });
            }
        } else if loss_tracker.is_some() {
            commands.entity(entity).remove::<LossTracker>();
        }
    }
}

pub fn check_order_fulfillment(
    mut commands: Commands,
    time: Res<Time>,
    mut score: ResMut<Score>,
    mut active_order: ResMut<ActiveOrder>,
    mut fulfillment: ResMut<ActiveFulfillment>,
    sphere_query: Query<
        (Entity, &Sphere),
        (
            Without<crate::physics_rules::MergeCooldown>,
            Without<crate::physics_rules::DespawnDelay>,
            Without<Fulfilling>,
        ),
    >,
    all_spheres: Query<&Transform, With<Sphere>>,
    mut fulfillment_burst_events: MessageWriter<FulfillmentBurstEvent>,
    mut game_rng: ResMut<GameRng>,
) {
    if let Some(entity) = fulfillment.entity {
        fulfillment.timer.tick(time.delta());
        if fulfillment.timer.is_finished() {
            if let Ok(sphere_transform) = all_spheres.get(entity) {
                let position = sphere_transform.translation;
                commands.entity(entity).despawn();

                fulfillment_burst_events.write(FulfillmentBurstEvent {
                    position,
                    tier: active_order.target_tier,
                });

                score.total += core_math::get_order_points(active_order.target_tier);
                if active_order.target_tier > score.peak_tier {
                    score.peak_tier = active_order.target_tier;
                }

                score.completed_orders += 1;

                let w8 = (score.completed_orders * 10).min(40);
                let w6 = 60u32.saturating_sub(score.completed_orders * 10).max(15);
                let w5 = 5u32;
                let w7 = 100 - w5 - w6 - w8;

                use rand::Rng;
                let roll = game_rng.rng.random_range(0..100);
                active_order.target_tier = if roll < w5 {
                    5
                } else if roll < w5 + w6 {
                    6
                } else if roll < w5 + w6 + w7 {
                    7
                } else {
                    8
                };
            }

            fulfillment.entity = None;
        }
    } else {
        for (entity, sphere) in sphere_query.iter() {
            if sphere.tier == active_order.target_tier {
                fulfillment.entity = Some(entity);
                fulfillment.timer.reset();

                commands
                    .entity(entity)
                    .insert(Fulfilling)
                    .insert(RigidBody::Fixed)
                    .remove::<ActiveEvents>()
                    .remove::<Velocity>();

                break;
            }
        }
    }
}

pub fn reset_game_state(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut active_order: ResMut<ActiveOrder>,
    mut queue: ResMut<DispenserQueue>,
    mut fulfillment: ResMut<ActiveFulfillment>,
    sphere_query: Query<Entity, With<Sphere>>,
    game_tick: Option<ResMut<GameTick>>,
) {
    info!("Resetting game state!");
    for entity in sphere_query.iter() {
        commands.entity(entity).despawn();
    }

    score.total = 0;
    score.peak_tier = 0;
    score.completed_orders = 0;

    active_order.target_tier = 6;

    queue.current = 1;
    queue.next = 2;

    *fulfillment = ActiveFulfillment::default();

    if let Some(mut tick) = game_tick {
        tick.0 = 0;
    }
}

pub mod physics_rules;
pub mod replay;
pub use replay::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_loss_condition_grace_period() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(GameSettings {
            launcher_z: 12.0,
            ..default()
        });
        app.add_systems(Update, check_loss_condition);

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        let entity = app
            .world_mut()
            .spawn((Sphere { tier: 1 }, Transform::from_xyz(0.0, 0.0, 12.5)))
            .id();

        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_some());
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.3));
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.3));
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::GameOver
        );
    }

    #[test]
    fn test_loss_condition_recoil() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(GameSettings {
            launcher_z: 12.0,
            ..default()
        });
        app.add_systems(Update, check_loss_condition);

        let entity = app
            .world_mut()
            .spawn((Sphere { tier: 1 }, Transform::from_xyz(0.0, 0.0, 12.5)))
            .id();

        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_some());

        app.world_mut()
            .entity_mut(entity)
            .get_mut::<Transform>()
            .unwrap()
            .translation
            .z = 12.0;

        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_none());
    }

    #[test]
    fn test_order_fulfillment() {
        let mut app = App::new();
        app.add_message::<FulfillmentBurstEvent>();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });
        app.insert_resource(ActiveOrder { target_tier: 4 });
        app.insert_resource(ActiveFulfillment::default());
        app.insert_resource(GameRng::default());
        app.add_systems(Update, check_order_fulfillment);

        let sphere_entity = app
            .world_mut()
            .spawn((Sphere { tier: 4 }, Transform::default()))
            .id();

        app.update();

        assert!(app.world().get_entity(sphere_entity).is_ok());
        assert!(app
            .world()
            .entity(sphere_entity)
            .get::<Fulfilling>()
            .is_some());

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(1.3));

        app.update();

        assert!(app.world().get_entity(sphere_entity).is_err());

        let score = app.world().resource::<Score>();
        assert_eq!(score.total, 2000);
        assert_eq!(score.peak_tier, 4);
        assert_eq!(score.completed_orders, 1);

        let order = app.world().resource::<ActiveOrder>();
        assert!(order.target_tier >= 5 && order.target_tier <= 8);
    }

    #[test]
    fn test_restart_from_game_over() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(GameSettings {
            launcher_z: 12.0,
            ..default()
        });
        app.insert_resource(Score::default());
        app.insert_resource(ActiveOrder { target_tier: 6 });
        app.insert_resource(DispenserQueue {
            current: 1,
            next: 2,
        });
        app.insert_resource(ActiveFulfillment::default());

        app.add_systems(OnEnter(AppState::InGame), reset_game_state);
        app.add_systems(Update, check_loss_condition);

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        let sphere_entity = app
            .world_mut()
            .spawn((Sphere { tier: 1 }, Transform::from_xyz(0.0, 0.0, 12.5)))
            .id();

        app.update();
        assert!(app
            .world()
            .entity(sphere_entity)
            .get::<LossTracker>()
            .is_some());

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.6));
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::GameOver
        );

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);

        app.update();

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        assert!(app.world().get_entity(sphere_entity).is_err());
    }

    #[test]
    fn test_loss_condition_fallen_y() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(GameSettings {
            launcher_z: 12.0,
            ..default()
        });
        app.add_systems(Update, check_loss_condition);

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        let entity = app
            .world_mut()
            .spawn((Sphere { tier: 1 }, Transform::from_xyz(0.0, -0.3, 12.1)))
            .id();

        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_some());

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.6));
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::GameOver
        );
    }
}
