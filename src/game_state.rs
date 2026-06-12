use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
    GameOver,
    Win,
}

use bevy_rapier3d::prelude::{ActiveEvents, RigidBody, Velocity};

#[derive(Component)]
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
            Without<crate::physics::MergeCooldown>,
            // Merged parents keep `Sphere` during their despawn-delay window;
            // without this filter, fulfillment could latch onto an entity that is
            // about to be despawned, stalling the order pipeline for a full timer.
            Without<crate::physics::DespawnDelay>,
            Without<Fulfilling>,
        ),
    >,
    all_spheres: Query<&Transform, With<Sphere>>,
    mut fulfillment_burst_events: MessageWriter<FulfillmentBurstEvent>,
) {
    if let Some(entity) = fulfillment.entity {
        fulfillment.timer.tick(time.delta());
        if fulfillment.timer.is_finished() {
            // Guard despawn: check if entity still exists before despawning and scoring
            if let Ok(sphere_transform) = all_spheres.get(entity) {
                let position = sphere_transform.translation;
                commands.entity(entity).despawn();

                fulfillment_burst_events.write(FulfillmentBurstEvent {
                    position,
                    tier: active_order.target_tier,
                });

                // Increment score and update peak tier
                score.total += crate::core_math::get_order_points(active_order.target_tier);
                if active_order.target_tier > score.peak_tier {
                    score.peak_tier = active_order.target_tier;
                }

                score.completed_orders += 1;

                // Assign a new order using a weighted dynamic progression:
                // - w5 (Tier 5): baseline 5% chance (helper order)
                // - w8 (Tier 8): starts at 0%, increases by 10% per completed order up to 40% max
                // - w6 (Tier 6): starts at 60%, decreases by 10% per completed order down to 15% min
                // - w7 (Tier 7): acts as the remainder to ensure the total weights always sum to exactly 100% (ranges between 35% and 40%)
                let w8 = (score.completed_orders * 10).min(40);
                let w6 = 60u32.saturating_sub(score.completed_orders * 10).max(15);
                let w5 = 5u32;
                let w7 = 100 - w5 - w6 - w8;

                let roll = rand::random_range(0..100);
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

pub fn reset_game_state(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut active_order: ResMut<ActiveOrder>,
    mut queue: ResMut<DispenserQueue>,
    mut fulfillment: ResMut<ActiveFulfillment>,
    sphere_query: Query<Entity, With<Sphere>>,
) {
    info!("Resetting game state!");
    // Despawn all gameplay spheres recursive
    for entity in sphere_query.iter() {
        commands.entity(entity).despawn();
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

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighScore(pub u32);

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualEffectsMode {
    #[default]
    On,
    Off,
}

/// Automatically switches visual effects off when the frame rate stays poor for
/// a sustained period during gameplay. The CRT pass plus alpha-blended sphere
/// shells are the dominant GPU cost, and a full board (high-score territory) is
/// the worst case — on weak GPUs this is the difference between a slowdown and
/// the browser killing the tab.
///
/// Fires at most once per session so it never fights a player who manually
/// re-enables FX afterward (F key or the FX button).
pub fn auto_degrade_visual_effects(
    time: Res<Time>,
    state: Res<State<AppState>>,
    mut effects_mode: ResMut<VisualEffectsMode>,
    mut over_budget: Local<f32>,
    mut already_fired: Local<bool>,
) {
    /// Frame-time budget: anything slower than 30 FPS counts as over budget.
    const FRAME_BUDGET: f32 = 1.0 / 30.0;
    /// Cumulative seconds of over-budget frames required to trigger.
    const TRIGGER_AFTER: f32 = 3.0;

    if *already_fired || *effects_mode == VisualEffectsMode::Off {
        return;
    }
    if *state.get() != AppState::InGame {
        return;
    }

    let dt = time.delta_secs();
    // Ignore one-off giant deltas (tab switch, window drag, shader-compile hitch)
    // so they can't trip the detector on their own.
    if dt > 1.0 {
        return;
    }

    if dt > FRAME_BUDGET {
        // Cap each frame's contribution so a few isolated spikes don't add up
        // as fast as genuinely sustained slowness.
        *over_budget += dt.min(0.1);
    } else {
        // Recover while the frame rate is healthy.
        *over_budget = (*over_budget - dt).max(0.0);
    }

    if *over_budget >= TRIGGER_AFTER {
        *effects_mode = VisualEffectsMode::Off;
        *already_fired = true;
        info!(
            "Sustained low frame rate detected — visual effects disabled automatically \
             (press F or the FX button to re-enable)."
        );
    }
}

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

#[cfg(target_arch = "wasm32")]
pub fn save_high_score_to_local_storage(val: u32) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item("akius_high_score", &val.to_string());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_high_score_to_local_storage(_val: u32) {}

#[cfg(target_arch = "wasm32")]
pub fn load_high_score_from_local_storage() -> u32 {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(Some(val_str)) = storage.get_item("akius_high_score") {
                if let Ok(val) = val_str.parse::<u32>() {
                    return val;
                }
            }
        }
    }
    0
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_high_score_from_local_storage() -> u32 {
    0
}

/// Minimum interval between localStorage writes while the high score keeps climbing.
const HIGH_SCORE_SAVE_INTERVAL: f32 = 5.0;

/// Tracks the in-memory high score every frame, but throttles the synchronous
/// localStorage write: once a player passes their previous best, *every* merge
/// raises the high score, and a blocking storage write per merge causes jank at
/// exactly the moment the game is busiest. `flush_high_score` guarantees the
/// final value is persisted on game over / win.
pub fn update_high_score(
    time: Res<Time>,
    score: Res<Score>,
    mut high_score: ResMut<HighScore>,
    mut dirty: Local<bool>,
    mut next_save_at: Local<f32>,
) {
    if score.total > high_score.0 {
        high_score.0 = score.total;
        *dirty = true;
    }
    if *dirty && time.elapsed_secs() >= *next_save_at {
        save_high_score_to_local_storage(high_score.0);
        *dirty = false;
        *next_save_at = time.elapsed_secs() + HIGH_SCORE_SAVE_INTERVAL;
    }
}

/// Persists the high score immediately. Run on entering GameOver/Win so the
/// throttling in `update_high_score` can never lose the final value.
pub fn flush_high_score(high_score: Res<HighScore>) {
    save_high_score_to_local_storage(high_score.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn auto_degrade_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(VisualEffectsMode::On);
        app.add_systems(Update, auto_degrade_visual_effects);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app
    }

    fn run_frames(app: &mut App, frames: usize, dt: f32) {
        for _ in 0..frames {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(dt));
            app.update();
        }
    }

    #[test]
    fn test_auto_degrade_fires_on_sustained_slowness() {
        let mut app = auto_degrade_test_app();
        // 25 FPS (40 ms frames) for ~4.4 seconds of game time
        run_frames(&mut app, 110, 0.04);
        assert_eq!(
            *app.world().resource::<VisualEffectsMode>(),
            VisualEffectsMode::Off
        );
    }

    #[test]
    fn test_auto_degrade_ignores_healthy_and_spiky_frames() {
        let mut app = auto_degrade_test_app();
        // Healthy 60 FPS frames never trigger it
        run_frames(&mut app, 600, 1.0 / 60.0);
        assert_eq!(
            *app.world().resource::<VisualEffectsMode>(),
            VisualEffectsMode::On
        );
        // A handful of giant deltas (tab switches) don't trigger it either
        run_frames(&mut app, 5, 5.0);
        assert_eq!(
            *app.world().resource::<VisualEffectsMode>(),
            VisualEffectsMode::On
        );
    }

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
        app.add_systems(Update, check_order_fulfillment);

        // Spawn matching sphere
        let sphere_entity = app
            .world_mut()
            .spawn((Sphere { tier: 4 }, Transform::default()))
            .id();

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

        // ActiveOrder should rotate to new target in scaled pool [5, 8] for completed_orders = 1
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

        // 3. Now transition back to InGame (simulating restart)
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);

        // This update should run StateTransition schedule, which transitions to InGame,
        // runs reset_game_state on OnEnter(InGame), and flushes commands.
        app.update();

        // Verify state is InGame
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        // Verify the sphere is despawned
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
