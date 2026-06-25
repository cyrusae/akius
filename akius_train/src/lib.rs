use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::{Py, PyAny};
use numpy::{IntoPyArray, PyArrayMethods};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::SeedableRng;

use akius_core::core_math;
use akius_core::physics_rules;
use akius_core::physics_rules::resolve_merges;
use akius_core::{
    AppState, Sphere, NextSphereId, GameRng, GameSettings,
    Score, ActiveOrder, DispenserQueue, ActiveFulfillment,
    FulfillmentBurstEvent, MergeBurstEvent, ColorblindMode, AimLineMode, QueuedInputs,
    check_loss_condition, check_order_fulfillment,
    // Re-exported from akius_core::replay via `pub use replay::*`
    ReplayPlugin, ReplayState, ReplayRecord, ShotRecord, CorrectionSnapshot, GameTick,
};
use serde::{Deserialize, Serialize};

#[derive(Resource, Default)]
pub struct TrainLauncherState {
    pub active_x: f32,
    pub cooldown_timer: Timer,
}

// ---------------------------------------------------------------------------
// Event log types — one merge/fulfillment record per game event
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct MergeRecord {
    pub tick: u64,
    pub tier: u8,
    pub x: f32,
    pub z: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FulfillmentRecord {
    pub tick: u64,
    pub tier: u8,
}

/// Per-step accumulator. Drained into the info dict at the end of each step().
#[derive(Resource, Default)]
struct TrainEventLog {
    merges: Vec<MergeRecord>,
    fulfillments: Vec<FulfillmentRecord>,
}

/// Full episode record: seed + shots + correction snapshots (for replay in the
/// rendered engine) plus the richer event log (for training metrics).
#[derive(Serialize, Deserialize)]
pub struct EpisodeRecord {
    pub seed: u64,
    pub final_score: u32,
    pub terminal_condition: String,
    pub shots: Vec<ShotRecord>,
    pub merges: Vec<MergeRecord>,
    pub fulfillments: Vec<FulfillmentRecord>,
    pub correction_snapshots: Vec<CorrectionSnapshot>,
}

/// Drains MergeBurstEvent messages into TrainEventLog each tick.
/// Ordered after resolve_merges, which is where those events are emitted.
fn log_merge_events(
    tick: Res<GameTick>,
    mut log: ResMut<TrainEventLog>,
    mut events: MessageReader<MergeBurstEvent>,
) {
    for event in events.read() {
        log.merges.push(MergeRecord {
            tick: tick.0,
            tier: event.tier,
            x: event.position.x,
            z: event.position.z,
        });
    }
}

/// Drains FulfillmentBurstEvent messages into TrainEventLog each tick.
/// Ordered after check_order_fulfillment, which emits those events.
fn log_fulfillment_events(
    tick: Res<GameTick>,
    mut log: ResMut<TrainEventLog>,
    mut events: MessageReader<FulfillmentBurstEvent>,
) {
    for event in events.read() {
        log.fulfillments.push(FulfillmentRecord {
            tick: tick.0,
            tier: event.tier,
        });
    }
}

#[pyclass(unsendable)]
pub struct PyGameEnv {
    app: App,
    previous_score: u32,
    ticks_since_last_shot: u32,
    // Episode-level accumulators — reset on each reset() call.
    episode_seed: u64,
    episode_shots: Vec<ShotRecord>,
    episode_merges: Vec<MergeRecord>,
    episode_fulfillments: Vec<FulfillmentRecord>,
}

fn check_obstruction(world: &mut World, target_x: f32, current_tier: u8) -> bool {
    let settings = world.resource::<GameSettings>();
    let r_launch = core_math::get_radius(current_tier);
    let launch_pos = Vec3::new(target_x, 0.0, settings.launcher_z);
    
    let mut query = world.query::<(&Transform, &Sphere)>();
    for (transform, sphere) in query.iter(world) {
        let r_sphere = core_math::get_radius(sphere.tier);
        let dz = (transform.translation.z - launch_pos.z).abs();
        let limit = r_sphere + r_launch + 0.02;
        if dz < limit {
            let dx = (transform.translation.x - launch_pos.x).abs();
            if dx < limit {
                let dist_sq = dx * dx + dz * dz;
                if dist_sq < limit * limit {
                    return true;
                }
            }
        }
    }
    false
}

impl PyGameEnv {
    fn get_observation_py<'py>(&mut self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let (dispenser_info, target_tier, completed_orders) = {
            let world = self.app.world();
            let dq = world.get_resource::<DispenserQueue>().map(|q| (q.current, q.next));
            let ao = world.get_resource::<ActiveOrder>().map(|o| o.target_tier);
            let s = world.get_resource::<Score>().map(|sc| sc.completed_orders);
            (dq, ao, s)
        };
        
        let world = self.app.world_mut();
        
        // 1. Build Spatial Grid (16x28x3)
        let mut grid_data = vec![0.0f32; 16 * 28 * 3];
        let mut query = world.query::<(&Transform, &Sphere, Option<&Velocity>)>();
        
        for (transform, sphere, velocity) in query.iter(world) {
            let s_x = transform.translation.x;
            let s_z = transform.translation.z;
            let r_sphere = core_math::get_radius(sphere.tier);
            
            let min_c = (((s_x - r_sphere + 4.0) / 0.5).floor() as i32).max(0) as usize;
            let max_c = (((s_x + r_sphere + 4.0) / 0.5).ceil() as i32).min(15) as usize;
            let min_r = (((s_z - r_sphere + 2.0) / 0.5).floor() as i32).max(0) as usize;
            let max_r = (((s_z + r_sphere + 2.0) / 0.5).ceil() as i32).min(27) as usize;
            
            let vel_z = velocity.map(|v| v.linear.z).unwrap_or(0.0);
            let speed = velocity.map(|v| v.linear.length()).unwrap_or(0.0);
            
            for row in min_r..=max_r {
                for col in min_c..=max_c {
                    let cell_x = -4.0 + (col as f32 + 0.5) * 0.5;
                    let cell_z = -2.0 + (row as f32 + 0.5) * 0.5;
                    let dx = cell_x - s_x;
                    let dz = cell_z - s_z;
                    if dx * dx + dz * dz <= r_sphere * r_sphere {
                        let idx = (row * 16 + col) * 3;
                        if grid_data[idx] == 0.0 || (sphere.tier as f32) > grid_data[idx] {
                            grid_data[idx] = sphere.tier as f32;
                            grid_data[idx + 1] = vel_z;
                            grid_data[idx + 2] = speed;
                        }
                    }
                }
            }
        }
        
        let py_grid = numpy::PyArray1::from_vec(py, grid_data)
            .reshape([16, 28, 3])?
            .into_any();
            
        // 2. Build Scalars (15-dim)
        let mut scalars = vec![0.0f32; 15];
        
        if let Some((current, next)) = dispenser_info {
            scalars[0] = current as f32;
            scalars[1] = next as f32;
        }
        
        if let Some(tier) = target_tier {
            scalars[2] = tier as f32;
        }
        
        if let Some(orders) = completed_orders {
            scalars[3] = orders as f32;
        }
        
        let mut count_query = world.query::<&Sphere>();
        for sphere in count_query.iter(world) {
            if sphere.tier == 5 { scalars[4] += 1.0; }
            else if sphere.tier == 6 { scalars[5] += 1.0; }
            else if sphere.tier == 7 { scalars[6] += 1.0; }
            else if sphere.tier == 8 { scalars[7] += 1.0; }
        }
        
        let mut ke_query = world.query::<(&Sphere, Option<&Velocity>)>();
        let mut total_ke = 0.0f32;
        let mut sphere_count = 0.0f32;
        for (sphere, velocity) in ke_query.iter(world) {
            let radius = core_math::get_radius(sphere.tier);
            let density = 15.0 / (sphere.tier as f32).powf(0.8);
            let volume = (4.0 / 3.0) * std::f32::consts::PI * radius.powi(3);
            let mass = density * volume;
            let vel_sq = velocity.map(|v| v.linear.length_squared()).unwrap_or(0.0);
            total_ke += 0.5 * mass * vel_sq;
            sphere_count += 1.0;
        }
        scalars[8] = if sphere_count > 0.0 { total_ke / sphere_count } else { 0.0 };
        
        let mut max_z = -2.0f32;
        let mut transform_query = world.query::<&Transform>();
        for transform in transform_query.iter(world) {
            if transform.translation.z > max_z {
                max_z = transform.translation.z;
            }
        }
        scalars[9] = max_z;
        
        let mut min_vz = 0.0f32;
        let mut in_flight_sphere = None;
        let mut in_flight_query = world.query::<(&Sphere, &Transform, &Velocity)>();
        for (sphere, transform, velocity) in in_flight_query.iter(world) {
            if velocity.linear.z < min_vz && velocity.linear.z < -1.0 {
                min_vz = velocity.linear.z;
                in_flight_sphere = Some((sphere, transform, velocity));
            }
        }
        if let Some((sphere, transform, velocity)) = in_flight_sphere {
            scalars[10] = sphere.tier as f32;
            scalars[11] = transform.translation.x;
            scalars[12] = transform.translation.z;
            scalars[13] = velocity.linear.z;
        }
        
        scalars[14] = self.ticks_since_last_shot as f32;
        
        let py_scalars = scalars.into_pyarray(py).into_any();
        
        Ok((py_grid, py_scalars))
    }
}

#[pymethods]
impl PyGameEnv {
    #[new]
    fn new() -> Self {
        let mut app = App::new();
        
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.add_plugins(bevy::transform::TransformPlugin);
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
        app.add_plugins(physics_rules::PhysicsPlugin);
        app.add_plugins(ReplayPlugin);
        
        app.init_state::<AppState>();
        
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualInstant(
            std::time::Instant::now()
        ));
        
        app.insert_resource(Time::<Fixed>::from_hz(60.0));
        app.insert_resource(GameSettings::default());
        app.insert_resource(Score::default());
        app.insert_resource(QueuedInputs::default());
        app.insert_resource(ColorblindMode(false));
        app.insert_resource(AimLineMode(false));
        app.insert_resource(ActiveOrder { target_tier: 6 });
        app.insert_resource(DispenserQueue {
            current: 1,
            next: 2,
        });
        app.insert_resource(ActiveFulfillment::default());
        app.insert_resource(TrainLauncherState {
            active_x: 0.0,
            cooldown_timer: Timer::from_seconds(0.4, TimerMode::Once),
        });
        app.add_message::<FulfillmentBurstEvent>();
        app.init_resource::<TrainEventLog>();

        // These two systems were missing from the original training setup.
        // Without them: episodes never end (no GameOver state), orders never
        // score (no fulfillment), and the dispenser order never advances.
        app.add_systems(
            FixedUpdate,
            (check_loss_condition, check_order_fulfillment)
                .after(PhysicsSet::Writeback)
                .run_if(in_state(AppState::InGame)),
        );

        // Event logging — drain Bevy message queues into TrainEventLog each tick,
        // ordered after the systems that emit those events.
        app.add_systems(
            FixedUpdate,
            log_merge_events
                .after(resolve_merges)
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            FixedUpdate,
            log_fulfillment_events
                .after(check_order_fulfillment)
                .run_if(in_state(AppState::InGame)),
        );

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);

        app.update();

        Self {
            app,
            previous_score: 0,
            ticks_since_last_shot: 0,
            episode_seed: 0,
            episode_shots: Vec::new(),
            episode_merges: Vec::new(),
            episode_fulfillments: Vec::new(),
        }
    }

    fn reset(&mut self, py: Python<'_>, seed: Option<u64>) -> PyResult<Py<PyAny>> {
        let world = self.app.world_mut();
        
        let mut spheres_to_despawn = Vec::new();
        let mut query = world.query_filtered::<Entity, With<Sphere>>();
        for entity in query.iter(world) {
            spheres_to_despawn.push(entity);
        }
        for entity in spheres_to_despawn {
            world.despawn(entity);
        }
        
        if let Some(mut score) = world.get_resource_mut::<Score>() {
            score.total = 0;
            score.peak_tier = 0;
            score.completed_orders = 0;
        }
        self.previous_score = 0;
        self.ticks_since_last_shot = 0;
        self.episode_shots.clear();
        self.episode_merges.clear();
        self.episode_fulfillments.clear();
        
        if let Some(mut next_id) = world.get_resource_mut::<NextSphereId>() {
            next_id.0 = 1;
        }
        
        if let Some(mut dispenser) = world.get_resource_mut::<DispenserQueue>() {
            dispenser.current = 1;
            dispenser.next = 2;
        }
        
        if let Some(mut active_order) = world.get_resource_mut::<ActiveOrder>() {
            active_order.target_tier = 6;
        }
        
        if let Some(mut fulfillment) = world.get_resource_mut::<ActiveFulfillment>() {
            fulfillment.entity = None;
            fulfillment.timer = Timer::from_seconds(1.2, TimerMode::Once);
        }
        
        if let Some(mut launcher) = world.get_resource_mut::<TrainLauncherState>() {
            launcher.active_x = 0.0;
            launcher.cooldown_timer = Timer::from_seconds(0.4, TimerMode::Once);
            launcher.cooldown_timer.tick(std::time::Duration::from_secs_f32(0.4));
        }
        
        if let Some(s) = seed {
            if let Some(mut game_rng) = world.get_resource_mut::<GameRng>() {
                game_rng.seed = s;
                game_rng.rng = rand::rngs::StdRng::seed_from_u64(s);
            }
        }

        // Clear the step-level event log from the previous episode.
        if let Some(mut log) = world.get_resource_mut::<TrainEventLog>() {
            log.merges.clear();
            log.fulfillments.clear();
        }

        // Start a fresh replay recording for this episode.
        let recording_seed = world.resource::<GameRng>().seed;
        *world.resource_mut::<ReplayState>() = ReplayState::Recording {
            record: ReplayRecord {
                seed: recording_seed,
                shots: vec![],
                correction_snapshots: vec![],
            },
        };

        world.resource_mut::<NextState<AppState>>().set(AppState::InGame);
        self.app.update();

        self.episode_seed = self.app.world().resource::<GameRng>().seed;

        let (grid, scalars) = self.get_observation_py(py)?;
        let info = PyDict::new(py);
        let obs = pyo3::types::PyTuple::new(py, &[grid, scalars])?;
        let result = pyo3::types::PyTuple::new(py, &[obs.into_any(), info.into_any()])?;
        Ok(result.into_any().unbind())
    }

    fn step(&mut self, py: Python<'_>, action: usize) -> PyResult<Py<PyAny>> {
        const HARD_CAP_TICKS: u32 = 480;

        // If the agent has been stalling too long, override to a forced center shot.
        // Bypasses obstruction below — the resulting collision is a valid game event.
        let mut action = action;
        let mut hard_cap_violated = false;
        if self.ticks_since_last_shot >= HARD_CAP_TICKS {
            action = 16; // X = 0.0
            hard_cap_violated = true;
        }

        let mut ticks_to_simulate = 24;
        let mut shot_fired = false;
        
        if action < 32 {
            let (target_x, launcher_z, launch_speed, current_tier, can_shoot) = {
                let world = self.app.world_mut();
                let settings = world.resource::<GameSettings>();
                let target_x = -4.0 + (action as f32 * 0.25);
                let launcher_z = settings.launcher_z;
                let launch_speed = settings.launch_speed;
                
                let current_tier = world
                    .get_resource::<DispenserQueue>()
                    .map(|dq| dq.current)
                    .unwrap_or(1);
                    
                let obstructed = !hard_cap_violated && check_obstruction(world, target_x, current_tier);
                
                let mut can_shoot = false;
                if let Some(launcher) = world.get_resource::<TrainLauncherState>() {
                    if launcher.cooldown_timer.is_finished() && !obstructed {
                        can_shoot = true;
                    }
                }
                (target_x, launcher_z, launch_speed, current_tier, can_shoot)
            };
            
            if can_shoot {
                let world = self.app.world_mut();
                let launch_position = Vec3::new(target_x, 0.0, launcher_z);
                let launch_velocity = Vec3::new(0.0, 0.0, -launch_speed);
                
                let id = {
                    let mut next_id = world.resource_mut::<NextSphereId>();
                    let val = next_id.0;
                    next_id.0 += 1;
                    val
                };
                
                physics_rules::spawn_sphere_entity(
                    &mut world.commands(),
                    id,
                    current_tier,
                    launch_position,
                    launch_velocity,
                );
                
                let next_tier = {
                    let mut game_rng = world.resource_mut::<GameRng>();
                    core_math::get_random_dispensed_tier(&mut game_rng.rng)
                };
                if let Some(mut queue) = world.get_resource_mut::<DispenserQueue>() {
                    queue.current = queue.next;
                    queue.next = next_tier;
                }
                
                if let Some(mut launcher) = world.get_resource_mut::<TrainLauncherState>() {
                    launcher.active_x = target_x;
                    launcher.cooldown_timer.reset();
                }

                // Tick is +1 because advance_game_tick fires at the START of the
                // next update, before physics processes the newly spawned sphere.
                let shot_tick = world.resource::<GameTick>().0 + 1;
                if let Some(mut rs) = world.get_resource_mut::<ReplayState>() {
                    if let ReplayState::Recording { record } = &mut *rs {
                        record.shots.push(ShotRecord { tick: shot_tick, x: target_x });
                    }
                }
                // world last used above — NLL ends the borrow before these self-field writes.
                shot_fired = true;
                self.ticks_since_last_shot = 0;
                self.episode_shots.push(ShotRecord { tick: shot_tick, x: target_x });
            }
            ticks_to_simulate = 24;
        } else if action == 32 {
            ticks_to_simulate = 6;
        } else if action == 33 {
            ticks_to_simulate = 12;
        } else if action == 34 {
            ticks_to_simulate = 24;
        }
        
        let mut accumulated_z_pressure = 0.0f32;
        let settings_launcher_z = {
            let world = self.app.world();
            world.resource::<GameSettings>().launcher_z
        };
        
        for _ in 0..ticks_to_simulate {
            let dt = 1.0 / 60.0;
            {
                let world = self.app.world_mut();
                if let Some(mut strategy) = world.get_resource_mut::<bevy::time::TimeUpdateStrategy>() {
                    if let bevy::time::TimeUpdateStrategy::ManualInstant(ref mut inst) = *strategy {
                        *inst += std::time::Duration::from_secs_f32(dt);
                    }
                }
                
                if let Some(mut launcher) = world.get_resource_mut::<TrainLauncherState>() {
                    launcher.cooldown_timer.tick(std::time::Duration::from_secs_f32(dt));
                }
            }
            
            self.app.update();
            
            if !shot_fired {
                self.ticks_since_last_shot += 1;
            }
            
            let mut max_z = -2.0f32;
            {
                let world = self.app.world_mut();
                let mut query = world.query::<&Transform>();
                for transform in query.iter(world) {
                    if transform.translation.z > max_z {
                        max_z = transform.translation.z;
                    }
                }
            }
            accumulated_z_pressure += (max_z / settings_launcher_z).clamp(0.0, 1.0);
        }
        
        // Drain the per-step event log — extend the episode accumulators at the same time.
        let (step_merges, step_fulfillments) = {
            let world = self.app.world_mut();
            let mut log = world.resource_mut::<TrainEventLog>();
            (std::mem::take(&mut log.merges), std::mem::take(&mut log.fulfillments))
        };
        self.episode_merges.extend(step_merges.iter().cloned());
        self.episode_fulfillments.extend(step_fulfillments.iter().cloned());

        let (current_score, win, loss) = {
            let world = self.app.world();
            let score = world.get_resource::<Score>().map(|s| s.total).unwrap_or(0);
            let app_state = *world.resource::<State<AppState>>().get();
            (score, app_state == AppState::Win, app_state == AppState::GameOver)
        };
        let score_delta = current_score.saturating_sub(self.previous_score);
        self.previous_score = current_score;

        let terminated = win || loss;
        let truncated = false;

        let (grid, scalars) = self.get_observation_py(py)?;
        let obs = pyo3::types::PyTuple::new(py, &[grid, scalars])?;

        let info = PyDict::new(py);
        info.set_item("score_delta", score_delta as f32)?;
        info.set_item("ticks_simulated", ticks_to_simulate as f32)?;
        info.set_item("accumulated_z_pressure", accumulated_z_pressure)?;
        info.set_item("win_condition_met", win)?;
        info.set_item("loss_condition_met", loss)?;
        info.set_item("hard_cap_violated", hard_cap_violated)?;

        // Per-step event lists — each entry is a dict with "tick", "tier", "x"/"z".
        let py_merges = pyo3::types::PyList::empty(py);
        for m in &step_merges {
            let d = PyDict::new(py);
            d.set_item("tick", m.tick)?;
            d.set_item("tier", m.tier as u32)?;
            d.set_item("x", m.x)?;
            d.set_item("z", m.z)?;
            py_merges.append(d)?;
        }
        info.set_item("merges", py_merges)?;

        let py_fulfillments = pyo3::types::PyList::empty(py);
        for f in &step_fulfillments {
            let d = PyDict::new(py);
            d.set_item("tick", f.tick)?;
            d.set_item("tier", f.tier as u32)?;
            py_fulfillments.append(d)?;
        }
        info.set_item("fulfillments", py_fulfillments)?;

        let tuple = pyo3::types::PyTuple::new(py, &[
            obs.into_any(),
            pyo3::types::PyFloat::new(py, 0.0).into_any(),
            pyo3::types::PyBool::new(py, terminated).to_owned().into_any(),
            pyo3::types::PyBool::new(py, truncated).to_owned().into_any(),
            info.into_any(),
        ])?;
        Ok(tuple.into_any().unbind())
    }

    /// Returns the full episode record as a JSON string.
    /// Call this once per episode, after the terminal step.
    /// The record contains the seed, shots, correction snapshots (for replay
    /// in the rendered engine), plus all merge and fulfillment events.
    fn episode_record_json(&self) -> PyResult<String> {
        let final_score;
        let terminal_condition;
        let correction_snapshots;
        {
            let world = self.app.world();
            final_score = world.get_resource::<Score>().map(|s| s.total).unwrap_or(0);
            let app_state = *world.resource::<State<AppState>>().get();
            terminal_condition = match app_state {
                AppState::Win => "win",
                AppState::GameOver => "game_over",
                _ => "truncated",
            }
            .to_string();
            correction_snapshots =
                if let Some(ReplayState::Recording { record }) = world.get_resource::<ReplayState>() {
                    record.correction_snapshots.clone()
                } else {
                    vec![]
                };
        }

        let record = EpisodeRecord {
            seed: self.episode_seed,
            final_score,
            terminal_condition,
            shots: self.episode_shots.clone(),
            merges: self.episode_merges.clone(),
            fulfillments: self.episode_fulfillments.clone(),
            correction_snapshots,
        };

        serde_json::to_string(&record)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
}

#[pymodule]
fn akius_train(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyGameEnv>()?;
    Ok(())
}
