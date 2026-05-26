use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    MainMenu,
    #[allow(dead_code)]
    InGame,
    GameOver,
}

use bevy_rapier3d::prelude::{ActiveEvents, RigidBody, Velocity};

#[derive(Component)]
pub struct Sphere {
    pub tier: u8,
}

#[derive(Component)]
pub struct InsideLauncher;

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

pub fn check_loss_condition(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sphere_query: Query<
        (Entity, &Transform, Option<&mut LossTracker>),
        (With<Sphere>, Without<InsideLauncher>, Without<Fulfilling>),
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
            Without<InsideLauncher>,
            Without<crate::physics::MergeCooldown>,
            Without<Fulfilling>,
        ),
    >,
) {
    if let Some(entity) = fulfillment.entity {
        fulfillment.timer.tick(time.delta());
        if fulfillment.timer.is_finished() {
            // Despawn the sphere fully
            commands.entity(entity).despawn();

            // Increment score and update peak tier
            score.total += crate::core_math::get_order_points(active_order.target_tier);
            if active_order.target_tier > score.peak_tier {
                score.peak_tier = active_order.target_tier;
            }

            score.completed_orders += 1;

            // Assign a new order using the scaling formula (starting at Tier 6)
            let min_tier = (6 + score.completed_orders / 2).min(8);
            let max_tier = (min_tier + 1 + (score.completed_orders % 2)).min(10);
            active_order.target_tier = rand::random_range(min_tier as u8..=max_tier as u8);

            // Finish fulfillment
            fulfillment.entity = None;
        }
    } else {
        for (entity, sphere) in sphere_query.iter() {
            if sphere.tier == active_order.target_tier {
                // Start fulfillment
                fulfillment.entity = Some(entity);
                fulfillment.timer.reset();

                // Mark sphere as fulfilling, lock it in place as fixed/static, but keep the collider active
                commands
                    .entity(entity)
                    .insert(Fulfilling)
                    .insert(RigidBody::Fixed)
                    .remove::<ActiveEvents>()
                    .remove::<Velocity>();

                break; // Fulfill only one matching sphere at a time
            }
        }
    }
}

fn despawn_recursive_custom(
    commands: &mut Commands,
    entity: Entity,
    children_query: &Query<&Children>,
) {
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            despawn_recursive_custom(commands, child, children_query);
        }
    }
    commands.entity(entity).despawn();
}

pub fn reset_game_state(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut active_order: ResMut<ActiveOrder>,
    mut queue: ResMut<DispenserQueue>,
    mut fulfillment: ResMut<ActiveFulfillment>,
    sphere_query: Query<Entity, With<Sphere>>,
    children_query: Query<&Children>,
) {
    info!("Resetting game state!");
    // Despawn all gameplay spheres
    for entity in sphere_query.iter() {
        despawn_recursive_custom(&mut commands, entity, &children_query);
    }

    // Reset game resources to initial values
    score.total = 0;
    score.peak_tier = 0;
    score.completed_orders = 0;

    active_order.target_tier = 6;

    queue.current = 1;
    queue.next = 2;

    *fulfillment = ActiveFulfillment::default();
}

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

        // Transition to InGame state using Bevy's State system
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        // Spawn a sphere past the launcher_z + 0.2 boundary (Z = 12.5)
        let entity = app
            .world_mut()
            .spawn((Sphere { tier: 1 }, Transform::from_xyz(0.0, 0.0, 12.5)))
            .id();

        // Tick 1: LossTracker should be added
        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_some());
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        // Advance time by 0.3 seconds
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.3));
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        // Advance time by another 0.3 seconds (total 0.6s, exceeding 0.5s grace period)
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.3));
        app.update(); // System ticks timer and sets NextState(GameOver)
        app.update(); // StateTransition applies state change

        // State transition should be executed
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

        // Tick 1: Behind line -> LossTracker added
        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_some());

        // Move sphere in front of line (Z = 12.0, below 12.2)
        app.world_mut()
            .entity_mut(entity)
            .get_mut::<Transform>()
            .unwrap()
            .translation
            .z = 12.0;

        // Tick 2: In front -> LossTracker removed
        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_none());
    }

    #[test]
    fn test_loss_condition_launcher_exclusion() {
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
            .spawn((
                Sphere { tier: 1 },
                InsideLauncher,
                Transform::from_xyz(0.0, 0.0, 12.5),
            ))
            .id();

        // Update -> Should NOT add LossTracker because InsideLauncher is present
        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_none());
    }

    #[test]
    fn test_order_fulfillment() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
            completed_orders: 0,
        });
        app.insert_resource(ActiveOrder { target_tier: 4 });
        app.insert_resource(ActiveFulfillment::default());
        app.add_systems(Update, check_order_fulfillment);

        // Spawn matching sphere
        let sphere_entity = app.world_mut().spawn(Sphere { tier: 4 }).id();

        // Run frame 1: Should detect matching sphere and start fulfillment
        app.update();

        // Sphere should still exist but have Fulfilling component
        assert!(app.world().get_entity(sphere_entity).is_ok());
        assert!(app
            .world()
            .entity(sphere_entity)
            .get::<Fulfilling>()
            .is_some());

        // Advance time by 1.3 seconds to complete the 1.2s fulfillment
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(1.3));

        // Run frame 2: Should complete fulfillment and despawn the sphere
        app.update();

        // Sphere should be despawned
        assert!(app.world().get_entity(sphere_entity).is_err());

        // Score should be incremented (Tier 4 completion = 2000 points)
        let score = app.world().resource::<Score>();
        assert_eq!(score.total, 2000);
        assert_eq!(score.peak_tier, 4);
        assert_eq!(score.completed_orders, 1);

        // ActiveOrder should rotate to new target in scaled pool [6, 8] for completed_orders = 1
        let order = app.world().resource::<ActiveOrder>();
        assert!(order.target_tier >= 6 && order.target_tier <= 8);
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

        app.add_systems(OnEnter(AppState::MainMenu), reset_game_state);
        app.add_systems(Update, check_loss_condition);

        // Transition to InGame so we don't trigger reset_game_state on the first update
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        // Spawn a sphere past the launcher line
        let sphere_entity = app
            .world_mut()
            .spawn((Sphere { tier: 1 }, Transform::from_xyz(0.0, 0.0, 12.5)))
            .id();

        // 1. Tick: adds LossTracker
        app.update();
        assert!(app
            .world()
            .entity(sphere_entity)
            .get::<LossTracker>()
            .is_some());

        // 2. Advance time past 0.5s to trigger GameOver
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.6));
        app.update(); // Set next state to GameOver
        app.update(); // Transition to GameOver
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::GameOver
        );

        // 3. Now transition back to MainMenu (simulating restart)
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::MainMenu);

        // This update should run StateTransition schedule, which transitions to MainMenu,
        // runs reset_game_state on OnEnter(MainMenu), and flushes commands.
        app.update();

        // Verify state is MainMenu
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::MainMenu
        );

        // Verify the sphere is despawned
        assert!(app.world().get_entity(sphere_entity).is_err());

        // Run another update to ensure it does not immediately transition back to GameOver
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::MainMenu
        );
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

        // Spawn a sphere that has rolled past settings.launcher_z (Z = 12.1) but hasn't reached Z = 12.2,
        // but has dropped vertically (Y = -0.3)
        let entity = app
            .world_mut()
            .spawn((Sphere { tier: 1 }, Transform::from_xyz(0.0, -0.3, 12.1)))
            .id();

        // Tick 1: should add LossTracker
        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_some());

        // Advance time past 0.5s to trigger GameOver
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.6));
        app.update(); // Set next state to GameOver
        app.update(); // Transition to GameOver

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::GameOver
        );
    }
}
