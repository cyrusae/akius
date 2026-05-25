mod core_math;
mod game_state;
mod launcher;
mod physics;
mod visuals;
mod hud;

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
        .insert_resource(game_state::GameSettings::default())
        .insert_resource(game_state::Score::default())
        .insert_resource(game_state::ColorblindMode::default())
        .insert_resource(game_state::ActiveOrder { target_tier: 4 })
        .insert_resource(game_state::DispenserQueue { current: 1, next: 2 })
        .add_systems(Startup, setup_camera_and_light)
        .run();
}

/// Camera and lighting only — scene geometry is handled by VisualPlugin.
fn setup_camera_and_light(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 15.0, 18.0).looking_at(Vec3::new(0.0, 0.0, 5.0), Vec3::Y),
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
