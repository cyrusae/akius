use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
    GameOver,
}

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

#[derive(Resource, Clone)]
pub struct GameSettings {
    pub loss_boundary_z: f32,
    pub launcher_z: f32,
    pub arena_width: f32,
    pub launch_speed: f32,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            loss_boundary_z: 10.0,
            launcher_z: 12.0,
            arena_width: 8.0,
            launch_speed: 15.0,
        }
    }
}

#[derive(Resource, Default, Clone)]
pub struct Score {
    pub total: u32,
    pub peak_tier: u8,
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

pub fn check_loss_condition(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sphere_query: Query<
        (Entity, &Transform, Option<&mut LossTracker>),
        (With<Sphere>, Without<InsideLauncher>),
    >,
) {
    for (entity, transform, loss_tracker) in sphere_query.iter_mut() {
        if transform.translation.z > settings.loss_boundary_z {
            if let Some(mut tracker) = loss_tracker {
                tracker.timer.tick(time.delta());
                if tracker.timer.is_finished() {
                    next_state.set(AppState::GameOver);
                }
            } else {
                commands.entity(entity).insert(LossTracker {
                    timer: Timer::from_seconds(1.5, TimerMode::Once),
                });
            }
        } else if loss_tracker.is_some() {
            commands.entity(entity).remove::<LossTracker>();
        }
    }
}

pub fn check_order_fulfillment(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut active_order: ResMut<ActiveOrder>,
    sphere_query: Query<(Entity, &Sphere), Without<InsideLauncher>>,
) {
    for (entity, sphere) in sphere_query.iter() {
        if sphere.tier == active_order.target_tier {
            // Despawn matching target sphere
            commands.entity(entity).despawn();

            // Increment score and update peak tier
            score.total += crate::core_math::get_order_points(active_order.target_tier);
            if sphere.tier > score.peak_tier {
                score.peak_tier = sphere.tier;
            }

            // Assign a new order from the initial pool [3, 6]
            active_order.target_tier = rand::random_range(3..=6);
            break; // Process one completion per frame
        }
    }
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
            loss_boundary_z: 10.0,
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

        // Spawn a sphere behind the Z boundary
        let entity = app
            .world_mut()
            .spawn((Sphere { tier: 1 }, Transform::from_xyz(0.0, 0.0, 12.0)))
            .id();

        // Tick 1: LossTracker should be added
        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_some());
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        // Advance time by 1.0 second
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(1.0));
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        // Advance time by another 0.6 seconds (total 1.6s, exceeding 1.5s grace period)
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.6));
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
            loss_boundary_z: 10.0,
            ..default()
        });
        app.add_systems(Update, check_loss_condition);

        let entity = app
            .world_mut()
            .spawn((Sphere { tier: 1 }, Transform::from_xyz(0.0, 0.0, 12.0)))
            .id();

        // Tick 1: Behind line -> LossTracker added
        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_some());

        // Move sphere in front of line
        app.world_mut()
            .entity_mut(entity)
            .get_mut::<Transform>()
            .unwrap()
            .translation
            .z = 8.0;

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
            loss_boundary_z: 10.0,
            ..default()
        });
        app.add_systems(Update, check_loss_condition);

        let entity = app
            .world_mut()
            .spawn((
                Sphere { tier: 1 },
                InsideLauncher,
                Transform::from_xyz(0.0, 0.0, 12.0),
            ))
            .id();

        // Update -> Should NOT add LossTracker because InsideLauncher is present
        app.update();
        assert!(app.world().entity(entity).get::<LossTracker>().is_none());
    }

    #[test]
    fn test_order_fulfillment() {
        let mut app = App::new();
        app.insert_resource(Score {
            total: 0,
            peak_tier: 0,
        });
        app.insert_resource(ActiveOrder { target_tier: 4 });
        app.add_systems(Update, check_order_fulfillment);

        // Spawn matching sphere
        let sphere_entity = app.world_mut().spawn(Sphere { tier: 4 }).id();

        // Run frame
        app.update();

        // Sphere should be despawned
        assert!(app.world().get_entity(sphere_entity).is_err());

        // Score should be incremented (Tier 4 completion = 2000 points)
        let score = app.world().resource::<Score>();
        assert_eq!(score.total, 2000);
        assert_eq!(score.peak_tier, 4);

        // ActiveOrder should rotate to new target in initial pool [3, 6]
        let order = app.world().resource::<ActiveOrder>();
        assert!(order.target_tier >= 3 && order.target_tier <= 6);
    }
}
