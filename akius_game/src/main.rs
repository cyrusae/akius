#![allow(clippy::type_complexity, clippy::too_many_arguments)]

mod hud;
mod launcher;
mod utils;
mod visuals;
mod crt_post_process;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use akius_core::*;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "akiuS".into(),
                        canvas: Some("#bevy-canvas".to_string()),
                        fit_canvas_to_parent: true,
                        #[cfg(target_arch = "wasm32")]
                        resolution: bevy::window::WindowResolution::default()
                            .with_scale_factor_override(1.0),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    meta_check: bevy::asset::AssetMetaCheck::Never,
                    ..default()
                }),
        )
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(akius_core::physics_rules::PhysicsPlugin)
        .add_plugins(akius_core::replay::ReplayPlugin)
        .add_plugins(launcher::LauncherPlugin)
        .add_plugins(visuals::VisualPlugin)
        .add_plugins(hud::HudPlugin)
        .add_plugins(crt_post_process::CrtPostProcessPlugin)
        .init_state::<AppState>()
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(GameSettings::default())
        .insert_resource(Score::default())
        .insert_resource(ColorblindMode::default())
        .insert_resource(AimLineMode::default())
        .insert_resource(visuals::VisualEffectsMode::default())
        .insert_resource(ActiveOrder { target_tier: 6 })
        .insert_resource(DispenserQueue {
            current: 1,
            next: 2,
        })
        .insert_resource(ActiveFulfillment::default())
        .insert_resource(visuals::HighScore(
            visuals::load_high_score_from_local_storage(),
        ))
        .add_message::<FulfillmentBurstEvent>()
        .add_systems(Startup, setup_camera_and_light)
        .add_systems(
            OnEnter(AppState::InGame),
            reset_game_state,
        )
        .add_systems(
            FixedUpdate,
            (
                check_loss_condition,
                check_order_fulfillment,
            )
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (
                visuals::update_high_score,
                visuals::auto_degrade_visual_effects,
                adjust_camera_fov,
            ),
        )
        .add_systems(
            OnEnter(AppState::GameOver),
            visuals::flush_high_score,
        )
        .add_systems(
            OnEnter(AppState::Win),
            visuals::flush_high_score,
        )
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
        let target_fov = if aspect_ratio < 1.0 {
            // Keep the horizontal field of view constant by adjusting vertical fov:
            // tan(fov_v / 2) = tan(fov_h_desired / 2) / aspect_ratio
            // Target a horizontal FOV half-angle tangent of 0.34 to keep board tight.
            let desired_fov_v = 2.0 * (0.34 / aspect_ratio).atan();
            desired_fov_v.clamp(std::f32::consts::FRAC_PI_4, 1.2)
        } else {
            std::f32::consts::FRAC_PI_4
        };
        // Only write on actual change: unconditional writes dirty Changed<Projection>
        // every frame and force downstream camera re-preparation.
        if (perspective.fov - target_fov).abs() > 1e-6 {
            perspective.fov = target_fov;
        }
    }
}

/// Camera and lighting only — scene geometry is handled by VisualPlugin.
fn setup_camera_and_light(mut commands: Commands) {
    // 3D perspective camera for the game scene.
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Msaa::Off,
        Transform::from_xyz(0.0, 15.0, 18.0).looking_at(Vec3::new(0.0, 0.0, 5.0), Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(4.0, 10.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
