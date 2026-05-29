#![allow(clippy::type_complexity, clippy::too_many_arguments)]

mod core_math;
mod game_state;
mod hud;
mod launcher;
mod physics;
mod utils;
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
        .insert_resource(game_state::HighScore(
            game_state::load_high_score_from_local_storage(),
        ))
        .add_message::<game_state::FulfillmentBurstEvent>()
        .add_systems(Startup, setup_camera_and_light)
        .add_systems(
            OnEnter(game_state::AppState::InGame),
            game_state::reset_game_state,
        )
        .add_systems(
            Update,
            (
                game_state::check_loss_condition,
                game_state::check_order_fulfillment,
            )
                .run_if(in_state(game_state::AppState::InGame)),
        )
        .add_systems(Update, (game_state::update_high_score, adjust_camera_fov))
        .run();
}

/// Dynamically adjusts the camera's vertical FOV on narrow screens to prevent horizontal clipping of the board.
fn adjust_camera_fov(
    window_query: Query<&Window>,
    mut camera_query: Query<&mut Projection, With<Camera3d>>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let Ok(mut projection) = camera_query.single_mut() else {
        return;
    };

    if let Projection::Perspective(ref mut perspective) = *projection {
        let aspect_ratio = window.width() / window.height();
        if aspect_ratio < 1.0 {
            // Keep the horizontal field of view constant by adjusting vertical fov:
            // tan(fov_v / 2) = tan(fov_h_desired / 2) / aspect_ratio
            // Target a horizontal FOV half-angle tangent of 0.5317.
            let desired_fov_v = 2.0 * (0.5317 / aspect_ratio).atan();
            perspective.fov = desired_fov_v.clamp(std::f32::consts::FRAC_PI_4, 1.6);
        } else {
            perspective.fov = std::f32::consts::FRAC_PI_4;
        }
    }
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
