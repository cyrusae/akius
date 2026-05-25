mod core_math;
mod game_state;
mod launcher;
mod physics;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some("#bevy-canvas".to_string()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(physics::PhysicsPlugin)
        .add_plugins(launcher::LauncherPlugin)
        .insert_resource(game_state::GameSettings::default())
        .insert_resource(game_state::Score::default())
        .insert_resource(game_state::ActiveOrder { target_tier: 4 })
        .insert_resource(game_state::DispenserQueue { current: 1, next: 2 })
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Spawn camera to view the table
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 15.0, 18.0).looking_at(Vec3::new(0.0, 0.0, 5.0), Vec3::Y),
    ));

    // Spawn directional light
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Spawn aiming preview entity
    let sphere_mesh = meshes.add(Sphere::new(1.0).mesh().uv(32, 18));
    commands.spawn((
        launcher::LauncherPreview,
        Mesh3d(sphere_mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.8, 0.8, 1.0, 0.5), // Semi-transparent
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 12.0),
    ));
}
