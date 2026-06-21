use akius_core::Sphere;
use akius_core::core_math;
use akius_core::physics_rules;
use akius_core::*;
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
        let mut cooldown_timer = Timer::from_seconds(0.4, TimerMode::Once);
        cooldown_timer.tick(std::time::Duration::from_secs_f32(0.4));
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
        app.init_resource::<LauncherState>()
            .init_resource::<QueuedInputs>()
            .add_systems(
                Update,
                (gather_launcher_inputs, update_launcher_preview_visuals)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                FixedUpdate,
                (
                    update_launcher_aiming,
                    check_launcher_obstructions,
                    handle_launch_input,
                )
                    .chain()
                    .after(PhysicsSet::Writeback)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

pub fn gather_launcher_inputs(
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    window_query: Query<&Window>,
    settings: Res<GameSettings>,
    dispenser_queue: Option<Res<DispenserQueue>>,
    touches: Res<Touches>,
    mut queued_inputs: ResMut<QueuedInputs>,
    interaction_query: Query<&Interaction>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut launcher_state: ResMut<LauncherState>,
    replay_state: Option<Res<ReplayState>>,
) {
    if let Some(state) = replay_state
        && matches!(*state, ReplayState::Playing { .. })
    {
        return;
    }
    let half_width = settings.arena_width * 0.5;
    let current_tier = dispenser_queue.map(|dq| dq.current).unwrap_or(1);
    let preview_radius = core_math::get_radius(current_tier);
    let limit_x = (half_width - preview_radius).max(0.0);

    let mut new_target_x = None;
    let mut current_input_position = None;
    let mut is_touch = false;

    if let Some(touch_pos) = touches.first_pressed_position() {
        current_input_position = Some(touch_pos);
        is_touch = true;
    } else if let Ok(window) = window_query.single() {
        current_input_position = window.cursor_position();
    }

    if let Some(input_pos) = current_input_position
        && let Ok((camera, camera_transform)) = camera_query.single()
        && let Ok(ray) = camera.viewport_to_world(camera_transform, input_pos)
    {
        let dir_y = ray.direction.y;
        if dir_y.abs() >= 1e-6 {
            let t = -ray.origin.y / dir_y;
            if t >= 0.0 {
                let intersection_point = ray.origin + t * *ray.direction;
                new_target_x = Some(intersection_point.x.clamp(-limit_x, limit_x));
            }
        }
    }

    let input_moved = if let Some(pos) = current_input_position {
        if is_touch {
            true
        } else if let Some(last_pos) = launcher_state.last_cursor_position {
            (pos - last_pos).length_squared() > 1e-4
        } else {
            true
        }
    } else {
        false
    };

    if input_moved {
        if let Some(x) = new_target_x {
            queued_inputs.target_x = Some(x);
        }
    } else {
        queued_inputs.target_x = None;
    }

    if !is_touch {
        launcher_state.last_cursor_position = current_input_position;
    } else {
        launcher_state.last_cursor_position = None;
    }

    let over_ui = interaction_query.iter().any(|&i| i != Interaction::None);
    let is_touch_active = touches.iter().next().is_some() || touches.any_just_released();

    for touch in touches.iter_just_pressed() {
        if !launcher_state.active_touch_ids.contains(&touch.id()) {
            launcher_state.active_touch_ids.push(touch.id());
        }
    }

    let mut touch_fired = false;
    for touch in touches.iter_just_released() {
        if let Some(pos) = launcher_state
            .active_touch_ids
            .iter()
            .position(|&id| id == touch.id())
        {
            touch_fired = true;
            launcher_state.active_touch_ids.swap_remove(pos);
        }
    }

    launcher_state
        .active_touch_ids
        .retain(|&id| touches.get_pressed(id).is_some());

    let fire_pressed = if is_touch_active {
        touch_fired && !over_ui
    } else {
        (mouse_button_input.just_pressed(MouseButton::Left) && !over_ui)
            || keyboard_input.just_pressed(KeyCode::Space)
    };

    if fire_pressed {
        queued_inputs.fire_requested = true;
    }
}

pub fn update_launcher_aiming(
    settings: Res<GameSettings>,
    dispenser_queue: Option<Res<DispenserQueue>>,
    sphere_query: Query<(&Transform, &Sphere, Option<&Velocity>), Without<Fulfilling>>,
    mut launcher_state: ResMut<LauncherState>,
    queued_inputs: Res<QueuedInputs>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut intervals: Local<Vec<(f32, f32)>>,
    mut merged_intervals: Local<Vec<(f32, f32)>>,
) {
    let half_width = settings.arena_width * 0.5;
    let current_tier = dispenser_queue.map(|dq| dq.current).unwrap_or(1);
    let preview_radius = core_math::get_radius(current_tier);
    let limit_x = (half_width - preview_radius).max(0.0);

    let mut clamped_x = launcher_state.target_x;
    if let Some(x) = queued_inputs.target_x {
        clamped_x = x;
    } else {
        let mut keyboard_dir = 0.0;
        if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
            keyboard_dir -= 1.0;
        }
        if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
            keyboard_dir += 1.0;
        }

        if keyboard_dir != 0.0 {
            let keyboard_speed = 8.0;
            clamped_x = (clamped_x + keyboard_dir * keyboard_speed * time.delta_secs())
                .clamp(-limit_x, limit_x);
        }
    }

    intervals.clear();
    for (sphere_transform, sphere, velocity) in sphere_query.iter() {
        if let Some(vel) = velocity
            && vel.linear.z < -0.5
        {
            continue;
        }
        let sphere_radius = core_math::get_radius(sphere.tier);
        let dz = (settings.launcher_z - sphere_transform.translation.z).abs();
        let r_sum = preview_radius + sphere_radius + 0.02;
        if dz < r_sum {
            let dx_max = (r_sum * r_sum - dz * dz).sqrt();
            let min_x = sphere_transform.translation.x - dx_max;
            let max_x = sphere_transform.translation.x + dx_max;
            intervals.push((min_x, max_x));
        }
    }

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
    clamped_x = clamped_x.clamp(-limit_x, limit_x);

    launcher_state.target_x = clamped_x;

    let dt = time.delta_secs();
    let lerp_factor = (1.0f32 - (-9.75f32 * dt).exp()).clamp(0.0f32, 1.0f32);
    launcher_state.active_x += (clamped_x - launcher_state.active_x) * lerp_factor;
}

pub fn check_launcher_obstructions(
    settings: Res<GameSettings>,
    dispenser_queue: Option<Res<DispenserQueue>>,
    mut launcher_state: ResMut<LauncherState>,
    rapier_context: ReadRapierContext,
    sphere_query: Query<&Sphere>,
    mut cached_shape: Local<Option<(u8, Collider)>>,
) {
    let Ok(context) = rapier_context.single() else {
        return;
    };

    let current_tier = dispenser_queue.map(|dq| dq.current).unwrap_or(1);
    if cached_shape.as_ref().map(|(tier, _)| *tier) != Some(current_tier) {
        let radius = core_math::get_radius(current_tier);
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
                return false;
            }
            true
        },
    );

    launcher_state.obstructed = obstructed;
}

pub fn handle_launch_input(
    mut commands: Commands,
    settings: Res<GameSettings>,
    dispenser_queue: Option<ResMut<DispenserQueue>>,
    mut launcher_state: ResMut<LauncherState>,
    mut queued_inputs: ResMut<QueuedInputs>,
    time: Res<Time>,
    merge_cooldowns: Query<(), With<physics_rules::MergeCooldown>>,
    mut next_id: ResMut<NextSphereId>,
    mut game_rng: ResMut<GameRng>,
    tick: Option<Res<GameTick>>,
    mut replay_state: Option<ResMut<ReplayState>>,
) {
    launcher_state.cooldown_timer.tick(time.delta());

    let merge_active = !merge_cooldowns.is_empty();

    if queued_inputs.fire_requested {
        queued_inputs.fire_requested = false;

        if launcher_state.cooldown_timer.is_finished() && !merge_active {
            let current_tier = dispenser_queue.as_ref().map(|dq| dq.current).unwrap_or(1);
            let launch_position = Vec3::new(launcher_state.active_x, 0.0, settings.launcher_z);
            let launch_velocity = Vec3::new(0.0, 0.0, -settings.launch_speed);

            let id = next_id.0;
            next_id.0 += 1;

            physics_rules::spawn_sphere_entity(
                &mut commands,
                id,
                current_tier,
                launch_position,
                launch_velocity,
            );

            if let Some(t) = tick
                && let Some(ref mut state) = replay_state
                && let ReplayState::Recording { record } = &mut **state
            {
                record.shots.push(ShotRecord {
                    tick: t.0,
                    x: launcher_state.active_x,
                });
            }

            if let Some(mut queue) = dispenser_queue {
                queue.current = queue.next;
                queue.next = core_math::get_random_dispensed_tier(&mut game_rng.rng);
            }

            launcher_state.cooldown_timer.reset();
        }
    }
}

pub fn update_launcher_preview_visuals(
    launcher_state: Res<LauncherState>,
    settings: Res<GameSettings>,
    dispenser_queue: Option<Res<DispenserQueue>>,
    mut preview_query: Query<(&mut Transform, &mut Visibility), With<LauncherPreview>>,
) {
    let Ok((mut transform, mut visibility)) = preview_query.single_mut() else {
        return;
    };

    let current_tier = dispenser_queue.map(|dq| dq.current).unwrap_or(1);
    let radius = core_math::get_radius(current_tier);

    transform.translation = Vec3::new(launcher_state.active_x, radius, settings.launcher_z);
    transform.scale = Vec3::splat(radius);

    let elapsed = launcher_state.cooldown_timer.elapsed_secs();
    if !launcher_state.cooldown_timer.is_finished() && elapsed < 0.2 {
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
        app.insert_resource(QueuedInputs::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
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

        app.add_systems(
            Update,
            (gather_launcher_inputs, update_launcher_aiming).chain(),
        );
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

        physics_rules::spawn_sphere_entity(
            &mut app.world_mut().commands(),
            1, // ID
            1, // Tier
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
        app.insert_resource(QueuedInputs::default());
        app.insert_resource(DispenserQueue {
            current: 3,
            next: 4,
        });
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Touches::default());
        app.insert_resource(Time::<()>::default());
        app.init_resource::<NextSphereId>();
        app.init_resource::<GameRng>();
        app.add_systems(
            Update,
            (gather_launcher_inputs, handle_launch_input).chain(),
        );

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
                    Vec3::new(2.0, core_math::get_radius(3), 12.0)
                );
                assert_eq!(velocity.linear, Vec3::new(0.0, 0.0, -15.0));
                found = true;
            }
        }
        assert!(found, "Launched sphere not found in world");

        let queue = app.world().resource::<DispenserQueue>();
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
        app.insert_resource(QueuedInputs::default());
        app.insert_resource(DispenserQueue {
            current: 1,
            next: 2,
        });
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Touches::default());
        app.insert_resource(Time::<()>::default());
        app.init_resource::<NextSphereId>();
        app.init_resource::<GameRng>();
        app.add_systems(
            Update,
            (gather_launcher_inputs, handle_launch_input).chain(),
        );

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.update();

        assert_eq!(app.world().resource::<DispenserQueue>().current, 2);

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.update();

        assert_eq!(app.world().resource::<DispenserQueue>().current, 2);
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
        app.insert_resource(QueuedInputs::default());
        app.insert_resource(DispenserQueue {
            current: 1,
            next: 2,
        });
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Touches::default());
        app.insert_resource(Time::<()>::default());
        app.init_resource::<NextSphereId>();
        app.init_resource::<GameRng>();
        app.add_systems(
            Update,
            (gather_launcher_inputs, handle_launch_input).chain(),
        );

        app.world_mut().spawn(physics_rules::MergeCooldown {
            timer: Timer::from_seconds(0.5, TimerMode::Once),
        });

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.update();

        assert_eq!(app.world().resource::<DispenserQueue>().current, 1);
    }

    #[test]
    fn test_touch_start_origin_filter() {
        use bevy::input::touch::{TouchInput, TouchPhase};
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::input::InputPlugin);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        app.insert_resource(GameSettings::default());
        app.insert_resource(LauncherState::default());
        app.insert_resource(QueuedInputs::default());
        app.insert_resource(DispenserQueue {
            current: 1,
            next: 2,
        });
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Time::<()>::default());
        app.init_resource::<NextSphereId>();
        app.init_resource::<GameRng>();

        app.add_systems(
            Update,
            (gather_launcher_inputs, handle_launch_input)
                .chain()
                .run_if(in_state(AppState::InGame)),
        );

        app.world_mut()
            .resource_mut::<Messages<TouchInput>>()
            .write(TouchInput {
                id: 42,
                phase: TouchPhase::Started,
                position: Vec2::new(100.0, 100.0),
                force: None,
                window: Entity::PLACEHOLDER,
            });
        app.update();

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        app.world_mut()
            .resource_mut::<Messages<TouchInput>>()
            .write(TouchInput {
                id: 42,
                phase: TouchPhase::Ended,
                position: Vec2::new(100.0, 100.0),
                force: None,
                window: Entity::PLACEHOLDER,
            });
        app.update();

        assert_eq!(app.world().resource::<DispenserQueue>().current, 1);

        app.world_mut()
            .resource_mut::<Messages<TouchInput>>()
            .write(TouchInput {
                id: 43,
                phase: TouchPhase::Started,
                position: Vec2::new(100.0, 100.0),
                force: None,
                window: Entity::PLACEHOLDER,
            });
        app.update();

        app.world_mut()
            .resource_mut::<Messages<TouchInput>>()
            .write(TouchInput {
                id: 43,
                phase: TouchPhase::Ended,
                position: Vec2::new(100.0, 100.0),
                force: None,
                window: Entity::PLACEHOLDER,
            });
        app.update();

        assert_eq!(app.world().resource::<DispenserQueue>().current, 2);
    }

    #[test]
    fn test_deterministic_simulation_playback() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::transform::TransformPlugin);
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
        app.add_plugins(physics_rules::PhysicsPlugin);
        app.add_plugins(LauncherPlugin);

        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();

        app.insert_resource(Time::<Fixed>::from_hz(60.0));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ));

        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(Touches::default());
        app.add_message::<FulfillmentBurstEvent>();

        app.insert_resource(GameSettings::default());
        app.insert_resource(Score::default());
        app.insert_resource(ActiveOrder { target_tier: 6 });
        app.insert_resource(ActiveFulfillment::default());

        app.insert_resource(DispenserQueue {
            current: 1,
            next: 2,
        });

        app.add_systems(OnEnter(AppState::InGame), reset_game_state);
        app.add_systems(
            FixedUpdate,
            (check_loss_condition, check_order_fulfillment)
                .after(PhysicsSet::Writeback)
                .run_if(in_state(AppState::InGame)),
        );

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        app.insert_resource(DispenserQueue {
            current: 1,
            next: 1,
        });

        for tick in 0..250 {
            if tick == 2 {
                let mut queued = app.world_mut().resource_mut::<QueuedInputs>();
                queued.target_x = Some(0.0);
                queued.fire_requested = true;
            }
            if tick == 3 {
                let mut queue = app.world_mut().resource_mut::<DispenserQueue>();
                queue.next = 1;
            }
            if tick == 45 {
                let mut queued = app.world_mut().resource_mut::<QueuedInputs>();
                queued.target_x = Some(0.0);
                queued.fire_requested = true;
            }

            app.update();
        }

        let total_score = app.world().resource::<Score>().total;
        let peak_tier = app.world().resource::<Score>().peak_tier;

        let mut query = app.world_mut().query::<(Entity, &Sphere, &Transform)>();
        let spheres = query.iter(app.world()).collect::<Vec<_>>();

        assert_eq!(total_score, 200);
        assert_eq!(peak_tier, 2);

        assert_eq!(spheres.len(), 1);
        let (_entity, sphere, transform) = spheres[0];
        assert_eq!(sphere.tier, 2);

        // Assert exact, byte-identical coordinates that demonstrate enhanced-determinism
        // Any mutation to physics damping, speed, or collision logic will break these values.
        assert_eq!(transform.translation.x, 0.0);
        assert_eq!(transform.translation.y, 0.605);
        assert_eq!(transform.translation.z, -5.553156);
    }
}
