use bevy::{
    core_pipeline::{
        core_3d::graph::Node3d,
        fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin},
    },
    prelude::*,
    render::{
        extract_component::ExtractComponent,
        render_graph::{InternedRenderLabel, RenderLabel},
        render_resource::ShaderType,
    },
    shader::ShaderRef,
};

pub const CRT_POST_PROCESS_SHADER_HANDLE: Handle<Shader> =
    bevy::asset::uuid_handle!("98374291-8734-9187-2391-287349187239");

#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default)]
pub struct CrtPostProcessSettings {
    pub time: f32,
    pub aspect_ratio: f32,
    pub glitch_intensity: f32,
    pub _padding: f32,
}

impl FullscreenMaterial for CrtPostProcessSettings {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(CRT_POST_PROCESS_SHADER_HANDLE)
    }

    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

pub struct CrtPostProcessPlugin;

impl Plugin for CrtPostProcessPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<CrtPostProcessSettings>::default())
            .add_systems(PreStartup, setup_crt_shader)
            .add_systems(Update, update_crt_settings);
    }
}

fn setup_crt_shader(mut shaders: ResMut<Assets<Shader>>) {
    let _ = shaders.insert(
        &CRT_POST_PROCESS_SHADER_HANDLE,
        Shader::from_wgsl(
            include_str!("../assets/shaders/crt_post_process.wgsl"),
            "shaders/crt_post_process.wgsl",
        ),
    );
}

fn update_crt_settings(
    time: Res<Time>,
    window_query: Query<&Window>,
    camera_query: Query<Entity, With<Camera3d>>,
    mut commands: Commands,
    effects_mode: Option<Res<crate::game_state::VisualEffectsMode>>,
    mut fulfill_events: MessageReader<crate::game_state::FulfillmentBurstEvent>,
    mut glitch_val: Local<f32>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let aspect_ratio = window.width() / window.height();
    let is_effects_on = effects_mode
        .map(|m| *m == crate::game_state::VisualEffectsMode::On)
        .unwrap_or(true);

    // Decay the full-screen glitch intensity over time
    *glitch_val = (*glitch_val - time.delta_secs() * 3.0).max(0.0);

    // Trigger full screen glitch wipe if an order was fulfilled
    for _ in fulfill_events.read() {
        *glitch_val = 1.0;
    }

    for entity in camera_query.iter() {
        if is_effects_on {
            commands.entity(entity).insert(CrtPostProcessSettings {
                time: time.elapsed_secs(),
                aspect_ratio,
                glitch_intensity: *glitch_val,
                _padding: 0.0,
            });
        } else {
            commands.entity(entity).remove::<CrtPostProcessSettings>();
        }
    }
}
