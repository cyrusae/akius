use crate::game_state::{GameSettings, Sphere};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

#[derive(Component, Debug, Clone, Copy)]
pub struct LauncherPreview;

#[derive(Resource, Debug)]
pub struct LauncherState {
    pub active_x: f32,
    pub target_x: f32,
    pub obstructed: bool,
    pub cooldown_timer: Timer,
    pub last_cursor_position: Option<Vec2>,
    pub active_touch_ids: Vec<u64>,
}

impl Default for LauncherState {
    fn default() -> Self {
        let mut cooldown_timer = Timer::from_seconds(0.8, TimerMode::Once);
        cooldown_timer.tick(std::time::Duration::from_secs_f32(0.8));
        Self {
            active_x: 0.0,
            target_x: 0.0,
            obstructed: false,
            cooldown_timer,
            last_cursor_position: None,
            active_touch_ids: Vec::new(),
        }
    }
}

pub struct LauncherPlugin;

impl Plugin for LauncherPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LauncherState>().add_systems(
            Update,
            (
                update_launcher_aiming,
                check_launcher_obstructions,
                handle_launch_input,
                update_launcher_preview_visuals,
            )
                .chain()
                .run_if(in_state(crate::game_state::AppState::InGame)),
        );
    }
}

/// Updates the aiming position by raycasting the screen cursor coordinates onto the Y=0 plane
/// and constraining the X coordinate to prevent the preview sphere from overlapping existing spheres.
/// Also handles A/D and Left/Right keyboard movement as alternative inputs.
pub fn update_launcher_aiming(
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    window_query: Query<&Window>,
    settings: Res<GameSettings>,
    dispenser_queue: Option<Res<crate::game_state::DispenserQueue>>,
    sphere_query: Query<
        (&Transform, &Sphere, Option<&Velocity>),
        Without<crate::game_state::Fulfilling>,
    >,
    mut launcher_state: ResMut<LauncherState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    time: Res<Time>,
    // Scratch buffers reused across frames to avoid per-frame heap allocations.
    mut intervals: Local<Vec<(f32, f32)>>,
    mut merged_intervals: Local<Vec<(f32, f32)>>,
) {
    let half_width = settings.arena_width * 0.5;
    let current_tier = dispenser_queue.map(|dq| dq.current).unwrap_or(1);
    let preview_radius = crate::core_math::get_radius(current_tier);
    let limit_x = (half_width - preview_radius).max(0.0);

    // 1. Get position from touch or cursor
    let mut new_target_x = None;
    let mut current_input_position = None;
    let mut is_touch = false;

    if let Some(touch_pos) = touches.first_pressed_position() {
        current_input_position = Some(touch_pos);
        is_touch = true;
    } else if let Ok(window) = window_query.single() {
        current_input_position = window.cursor_position();
    }

    if let Some(input_pos) = current_input_position {
        if let Ok((camera, camera_transform)) = camera_query.single() {
            if let Ok(ray) = camera.viewport_to_world(camera_transform, input_pos) {
                let dir_y = ray.direction.y;
                if dir_y.abs() >= 1e-6 {
                    let t = -ray.origin.y / dir_y;
                    if t >= 0.0 {
                        let intersection_point = ray.origin + t * *ray.direction;
                        new_target_x = Some(intersection_point.x.clamp(-limit_x, limit_x));
                    }
                }
            }
        }
    }

    let input_moved = if let Some(pos) = current_input_position {
        if is_touch {
            // Touch targeting directly controls launcher positioning
            true
        } else if let Some(last_pos) = launcher_state.last_cursor_position {
            (pos - last_pos).length_squared() > 1e-4
        } else {
            true
        }
    } else {
        false
    };

    // 2. Decide target_x source (input vs keyboard)
    let mut clamped_x = launcher_state.target_x;
    if input_moved {
        if let Some(x) = new_target_x {
            clamped_x = x;
        }
    } else {
        // Read keyboard input
        let mut keyboard_dir = 0.0;
        if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
            keyboard_dir -= 1.0;
        }
        if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
            keyboard_dir += 1.0;
        }

        if keyboard_dir != 0.0 {
            let keyboard_speed = 8.0; // speed units per second
            clamped_x = (clamped_x + keyboard_dir * keyboard_speed * time.delta_secs())
                .clamp(-limit_x, limit_x);
        }
    }

    // Always update last input position if not touch
    if !is_touch {
        launcher_state.last_cursor_position = current_input_position;
    } else {
        launcher_state.last_cursor_position = None;
    }

    // Compute 1D forbidden intervals on the launcher line (Z = settings.launcher_z)
    intervals.clear();
    for (sphere_transform, sphere, velocity) in sphere_query.iter() {
        if let Some(vel) = velocity {
            if vel.linear.z < -0.5 {
                // Sphere is moving away from the launcher, ignore it for aiming obstruction snap calculations
                continue;
            }
        }
        let sphere_radius = crate::core_math::get_radius(sphere.tier);
        let dz = (settings.launcher_z - sphere_transform.translation.z).abs();
        let r_sum = preview_radius + sphere_radius + 0.02; // Added 0.02 units visual padding buffer
        if dz < r_sum {
            // Overlap is possible along X
            let dx_max = (r_sum * r_sum - dz * dz).sqrt();
            let min_x = sphere_transform.translation.x - dx_max;
            let max_x = sphere_transform.translation.x + dx_max;
            intervals.push((min_x, max_x));
        }
    }

    // Merge overlapping intervals
    intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    merged_intervals.clear();
    for &interval in intervals.iter() {
        if let Some(last) = merged_intervals.last_mut() {
            if interval.0 <= last.1 {
                last.1 = last.1.max(interval.1);
            } else {
                merged_intervals.push(interval);
            }
        } else {
            merged_intervals.push(interval);
        }
    }

    // Adjust target X if it lies within a forbidden interval.
    // Snap only to options that are within the [-limit_x, limit_x] table boundaries.
    for &(a, b) in merged_intervals.iter() {
        if clamped_x > a && clamped_x < b {
            let left_valid = a >= -limit_x;
            let right_valid = b <= limit_x;

            if left_valid && right_valid {
                if (clamped_x - a).abs() < (clamped_x - b).abs() {
                    clamped_x = a;
                } else {
                    clamped_x = b;
                }
            } else if left_valid {
                clamped_x = a;
            } else if right_valid {
                clamped_x = b;
            }
        }
    }
    // Re-clamp to boundaries in case snapping pushed it out of range
    clamped_x = clamped_x.clamp(-limit_x, limit_x);

    launcher_state.target_x = clamped_x;

    let dt = time.delta_secs();
    let lerp_factor = (1.0f32 - (-9.75f32 * dt).exp()).clamp(0.0f32, 1.0f32);
    launcher_state.active_x += (clamped_x - launcher_state.active_x) * lerp_factor;
}

/// Checks if spawning the preview sphere at the active aim point overlaps settled spheres
pub fn check_launcher_obstructions(
    settings: Res<GameSettings>,
    dispenser_queue: Option<Res<crate::game_state::DispenserQueue>>,
    mut launcher_state: ResMut<LauncherState>,
    rapier_context: ReadRapierContext,
    sphere_query: Query<&Sphere>,
    // Cache the query shape per tier instead of heap-allocating a new Rapier
    // collider every frame.
    mut cached_shape: Local<Option<(u8, Collider)>>,
) {
    let Ok(context) = rapier_context.single() else {
        return;
    };

    let current_tier = dispenser_queue.map(|dq| dq.current).unwrap_or(1);
    if cached_shape.as_ref().map(|(tier, _)| *tier) != Some(current_tier) {
        let radius = crate::core_math::get_radius(current_tier);
        *cached_shape = Some((current_tier, Collider::ball(radius)));
    }
    let shape = &*cached_shape.as_ref().unwrap().1.raw;

    let position = Vec3::new(launcher_state.active_x, 0.0, settings.launcher_z);

    let mut obstructed = false;
    context.intersect_shape(
        position,
        Quat::IDENTITY,
        shape,
        QueryFilter::default(),
        |entity| {
            if sphere_query.contains(entity) {
                obstructed = true;
                return false; // Stop iterating
            }
            true // Continue checking other collisions
        },
    );

    launcher_state.obstructed = obstructed;
}

/// Ticks cooldown and spawns sphere on Left click or Space bar when unobstructed.
/// Prevents launching during active sphere merge animations.
pub fn handle_launch_input(
    mut commands: Commands,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    settings: Res<GameSettings>,
    dispenser_queue: Option<ResMut<crate::game_state::DispenserQueue>>,
    mut launcher_state: ResMut<LauncherState>,
    time: Res<Time>,
    interaction_query: Query<&Interaction>,
    merge_cooldowns: Query<(), With<crate::physics::MergeCooldown>>,
) {
    launcher_state.cooldown_timer.tick(time.delta());

    let over_ui = interaction_query.iter().any(|&i| i != Interaction::None);
    let is_touch_active = touches.iter().next().is_some() || touches.any_just_released();

    // Track touch lifecycle inside InGame
    for touch in touches.iter_just_pressed() {
        if !launcher_state.active_touch_ids.contains(&touch.id()) {
            launcher_state.active_touch_ids.push(touch.id());
        }
    }

    let mut touch_fired = false;
    for touch in touches.iter_just_released() {
        if let Some(pos) = launcher_state.active_touch_ids.iter().position(|&id| id == touch.id()) {
            touch_fired = true;
            launcher_state.active_touch_ids.swap_remove(pos);
        }
    }

    launcher_state.active_touch_ids.retain(|&id| touches.get_pressed(id).is_some());

    let fire_pressed = if is_touch_active {
        touch_fired && !over_ui
    } else {
        (mouse_button_input.just_pressed(MouseButton::Left) && !over_ui)
            || keyboard_input.just_pressed(KeyCode::Space)
    };

    let merge_active = !merge_cooldowns.is_empty();

    if fire_pressed && launcher_state.cooldown_timer.is_finished() && !merge_active {
        let current_tier = dispenser_queue.as_ref().map(|dq| dq.current).unwrap_or(1);
        let launch_position = Vec3::new(launcher_state.active_x, 0.0, settings.launcher_z);
        let launch_velocity = Vec3::new(0.0, 0.0, -settings.launch_speed);

        crate::physics::spawn_sphere_entity(
            &mut commands,
            current_tier,
            launch_position,
            launch_velocity,
        );

        if let Some(mut queue) = dispenser_queue {
            queue.current = queue.next;
            queue.next = crate::core_math::get_random_dispensed_tier(&mut rand::rng());
        }

        launcher_state.cooldown_timer.reset();
    }
}

/// Updates the visualization of the preview sphere entity
pub fn update_launcher_preview_visuals(
    launcher_state: Res<LauncherState>,
    settings: Res<GameSettings>,
    dispenser_queue: Option<Res<crate::game_state::DispenserQueue>>,
    mut preview_query: Query<(&mut Transform, &mut Visibility), With<LauncherPreview>>,
) {
    let Ok((mut transform, mut visibility)) = preview_query.single_mut() else {
        return;
    };

    let current_tier = dispenser_queue.map(|dq| dq.current).unwrap_or(1);
    let radius = crate::core_math::get_radius(current_tier);

    transform.translation = Vec3::new(launcher_state.active_x, radius, settings.launcher_z);
    transform.scale = Vec3::splat(radius);

    // Hide preview sphere during the first part of the cooldown for a smoother spawning feel
    let elapsed = launcher_state.cooldown_timer.elapsed_secs();
    if !launcher_state.cooldown_timer.is_finished() && elapsed < 0.4 {
        crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
    } else {
        crate::utils::set_visibility(&mut visibility, Visibility::Visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raycast_cursor_plane_intersection() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(GameSettings::default());
        app.insert_resource(LauncherState::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Touches::default());
        app.insert_resource(Time::<()>::default());

        let camera_transform = Transform::from_xyz(0.0, 10.0, 0.0).looking_at(Vec3::ZERO, -Vec3::Z);
        app.world_mut().spawn((
            Camera3d::default(),
            Camera {
                viewport: Some(bevy::camera::Viewport {
                    physical_position: UVec2::ZERO,
                    physical_size: UVec2::new(800, 600),
                    ..default()
                }),
                ..default()
            },
            camera_transform,
            GlobalTransform::from(camera_transform),
        ));

        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::new(400.0, 300.0)));
        app.world_mut().spawn(window);

        app.add_systems(Update, update_launcher_aiming);
        app.update();

        let state = app.world().resource::<LauncherState>();
        assert!((state.target_x - 0.0).abs() < 1e-4);
    }

    #[test]
    fn test_preview_obstruction_detection() {
        use bevy::time::TimePlugin;
        use bevy::transform::TransformPlugin;

        let mut app = App::new();
        app.add_plugins((
            TransformPlugin,
            TimePlugin,
            RapierPhysicsPlugin::<NoUserData>::default(),
        ));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ));
        app.insert_resource(GameSettings::default());
        app.insert_resource(LauncherState {
            active_x: 1.0,
            ..default()
        });
        app.add_systems(Update, check_launcher_obstructions);

        crate::physics::spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1,
            Vec3::new(1.0, 0.0, 12.0),
            Vec3::ZERO,
        );

        app.update();
        app.update();

        let state = app.world().resource::<LauncherState>();
        assert!(state.obstructed);

        app.world_mut().resource_mut::<LauncherState>().active_x = -2.0;
        app.update();

        let state = app.world().resource::<LauncherState>();
        assert!(!state.obstructed);
    }

    #[test]
    fn test_launch_spawn_and_impulse() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(GameSettings::default());
        app.insert_resource(LauncherState {
            active_x: 2.0,
            ..default()
        });
        app.insert_resource(crate::game_state::DispenserQueue {
            current: 3,
            next: 4,
        });
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Touches::default());
        app.insert_resource(Time::<()>::default());
        app.add_systems(Update, handle_launch_input);

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        app.update();
        app.update();

        let mut query = app
            .world_mut()
            .query::<(Entity, &Sphere, &Transform, &Velocity)>();
        let mut found = false;
        for (_entity, sphere, transform, velocity) in query.iter(app.world()) {
            if sphere.tier == 3 {
                assert_eq!(
                    transform.translation,
                    Vec3::new(2.0, crate::core_math::get_radius(3), 12.0)
                );
                assert_eq!(velocity.linear, Vec3::new(0.0, 0.0, -15.0));
                found = true;
            }
        }
        assert!(found, "Launched sphere not found in world");

        let queue = app.world().resource::<crate::game_state::DispenserQueue>();
        assert_eq!(queue.current, 4);
        assert!(queue.next >= 1 && queue.next <= 5);
    }

    #[test]
    fn test_launch_cooldown_blocking() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(GameSettings::default());
        app.insert_resource(LauncherState {
            active_x: 0.0,
            ..default()
        });
        app.insert_resource(crate::game_state::DispenserQueue {
            current: 1,
            next: 2,
        });
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Touches::default());
        app.insert_resource(Time::<()>::default());
        app.add_systems(Update, handle_launch_input);

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.update();

        assert_eq!(
            app.world()
                .resource::<crate::game_state::DispenserQueue>()
                .current,
            2
        );

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.update();

        assert_eq!(
            app.world()
                .resource::<crate::game_state::DispenserQueue>()
                .current,
            2
        );
    }

    #[test]
    fn test_merge_cooldown_blocking() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(GameSettings::default());
        app.insert_resource(LauncherState {
            active_x: 0.0,
            ..default()
        });
        app.insert_resource(crate::game_state::DispenserQueue {
            current: 1,
            next: 2,
        });
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Touches::default());
        app.insert_resource(Time::<()>::default());
        app.add_systems(Update, handle_launch_input);

        // Spawn a dummy entity with MergeCooldown to simulate active merge animation
        app.world_mut().spawn(crate::physics::MergeCooldown {
            timer: Timer::from_seconds(0.5, TimerMode::Once),
        });

        // Try to fire
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.update();

        // The launch should be blocked, queue.current remains 1
        assert_eq!(
            app.world()
                .resource::<crate::game_state::DispenserQueue>()
                .current,
            1
        );
    }

    #[test]
    fn test_touch_start_origin_filter() {
        use bevy::input::touch::{TouchInput, TouchPhase};
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::input::InputPlugin);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<crate::game_state::AppState>();
        app.insert_resource(GameSettings::default());
        app.insert_resource(LauncherState::default());
        app.insert_resource(crate::game_state::DispenserQueue {
            current: 1,
            next: 2,
        });
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Time::<()>::default());

        app.add_systems(
            Update,
            handle_launch_input.run_if(in_state(crate::game_state::AppState::InGame)),
        );

        // Case 1: Touch started before InGame (e.g. in MainMenu)
        app.world_mut().resource_mut::<Messages<TouchInput>>().write(TouchInput {
            id: 42,
            phase: TouchPhase::Started,
            position: Vec2::new(100.0, 100.0),
            force: None,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        // Transition to InGame
        app.world_mut()
            .resource_mut::<NextState<crate::game_state::AppState>>()
            .set(crate::game_state::AppState::InGame);
        app.update(); // Apply state transition

        // Release the touch that started in MainMenu
        app.world_mut().resource_mut::<Messages<TouchInput>>().write(TouchInput {
            id: 42,
            phase: TouchPhase::Ended,
            position: Vec2::new(100.0, 100.0),
            force: None,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        // Verify that the sphere did NOT launch (current remains 1)
        assert_eq!(
            app.world().resource::<crate::game_state::DispenserQueue>().current,
            1
        );

        // Case 2: Touch starts inside InGame
        app.world_mut().resource_mut::<Messages<TouchInput>>().write(TouchInput {
            id: 43,
            phase: TouchPhase::Started,
            position: Vec2::new(100.0, 100.0),
            force: None,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        app.world_mut().resource_mut::<Messages<TouchInput>>().write(TouchInput {
            id: 43,
            phase: TouchPhase::Ended,
            position: Vec2::new(100.0, 100.0),
            force: None,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        // Verify that the sphere DID launch (current becomes 2)
        assert_eq!(
            app.world().resource::<crate::game_state::DispenserQueue>().current,
            2
        );
    }
}
