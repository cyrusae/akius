mod core_math;
mod game_state;
mod hud;
mod launcher;
mod physics;
mod visuals;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "akiuS".into(),
                canvas: Some("#bevy-canvas".to_string()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(physics::PhysicsPlugin)
        .add_plugins(launcher::LauncherPlugin)
        .add_plugins(visuals::VisualPlugin)
        .add_plugins(hud::HudPlugin)
        .init_state::<game_state::AppState>()
        .insert_resource(game_state::GameSettings::default())
        .insert_resource(game_state::Score::default())
        .insert_resource(game_state::ColorblindMode::default())
        .insert_resource(game_state::AimLineMode::default())
        .insert_resource(game_state::ActiveOrder { target_tier: 6 })
        .insert_resource(game_state::DispenserQueue {
            current: 1,
            next: 2,
        })
        .insert_resource(game_state::ActiveFulfillment::default())
        .add_systems(Startup, setup_camera_and_light)
        .add_systems(
            OnEnter(game_state::AppState::MainMenu),
            game_state::reset_game_state,
        )
        .add_systems(
            Update,
            (
                game_state::check_loss_condition,
                game_state::check_order_fulfillment,
            )
                .run_if(not(in_state(game_state::AppState::GameOver))),
        )
        .run();
}

/// Camera and lighting only — scene geometry is handled by VisualPlugin.
fn setup_camera_and_light(mut commands: Commands) {
    // 3D perspective camera for the game scene.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 15.0, 18.0).looking_at(Vec3::new(0.0, 0.0, 5.0), Vec3::Y),
    ));

    // 2D overlay camera for screen-space colorblindness labels
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::None,
            order: 1,
            ..default()
        },
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 10.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
