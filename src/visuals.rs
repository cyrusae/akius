use crate::core_math::get_radius;
use crate::game_state::{AimLineMode, ColorblindMode, DispenserQueue, GameSettings, Sphere};
use crate::launcher::{LauncherPreview, LauncherState};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy_rapier3d::prelude::Collider;

// ---------------------------------------------------------------------------
// Tier color palette — vibrant rainbow gradient, designed to be distinct.
// ---------------------------------------------------------------------------
pub const TIER_COLORS: [Color; 9] = [
    Color::hsl(270.0, 0.70, 0.55), // Tier  1 — violet
    Color::hsl(225.0, 0.65, 0.60), // Tier  2 — blue
    Color::hsl(195.0, 0.70, 0.55), // Tier  3 — sky blue
    Color::hsl(150.0, 0.65, 0.50), // Tier  4 — teal/greenish
    Color::hsl(110.0, 0.60, 0.45), // Tier  5 — green
    Color::hsl(60.0, 0.75, 0.50),  // Tier  6 — yellow/amber
    Color::hsl(25.0, 0.85, 0.52),  // Tier  7 — orange
    Color::hsl(0.0, 0.85, 0.52),   // Tier  8 — red (max regular)
    Color::hsl(45.0, 0.95, 0.55),  // Tier  9 — gold (secret win)
];

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Tag on the mesh child entity attached to a physics `Sphere` entity.
#[derive(Component)]
pub struct SphereVisual;

/// Tag on the nested emissive core entity.
#[derive(Component)]
pub struct SphereCore;

/// Tag on the preview core entity.
#[derive(Component)]
pub struct PreviewCore;

/// Tag on falling Matrix particles.
#[derive(Component)]
pub struct MatrixParticle {
    pub velocity: Vec3,
    pub lifetime: Timer,
}

/// Tag on a root-level Text2d label entity. Stores the sphere entity it follows.
#[derive(Component)]
pub struct BillboardLabel(pub Entity);

/// Tag on a root-level Text2d label entity for the launcher preview sphere.
#[derive(Component)]
pub struct PreviewLabel;

/// Tag on the aiming guide line entity.
#[derive(Component)]
pub struct AimGuideLine;

// ---------------------------------------------------------------------------
// Resources & Custom Materials
// ---------------------------------------------------------------------------

pub const SPHERE_GRID_SHADER_HANDLE: Handle<Shader> =
    bevy::asset::uuid_handle!("28394710-9283-7498-1273-918231273491");
pub const FLOOR_GRID_SHADER_HANDLE: Handle<Shader> =
    bevy::asset::uuid_handle!("17283947-1928-3749-1827-394817293847");
pub const LASER_LINE_SHADER_HANDLE: Handle<Shader> =
    bevy::asset::uuid_handle!("38294710-9283-7498-1273-918231273492");
pub const RETICLE_SHADER_HANDLE: Handle<Shader> =
    bevy::asset::uuid_handle!("48294710-9283-7498-1273-918231273493");


#[derive(ShaderType, Clone, Copy, Debug)]
pub struct SphereUniforms {
    pub color: LinearRgba,
    pub base_color: LinearRgba,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SphereMaterial {
    #[uniform(0)]
    pub uniforms: SphereUniforms,
}

impl Material for SphereMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(SPHERE_GRID_SHADER_HANDLE)
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct FloorUniforms {
    pub grid_color: LinearRgba,
    pub bg_color: LinearRgba,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct FloorMaterial {
    #[uniform(0)]
    pub uniforms: FloorUniforms,
}

impl Material for FloorMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(FLOOR_GRID_SHADER_HANDLE)
    }
}

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct LaserUniforms {
    pub color: LinearRgba,
    pub time: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct LaserMaterial {
    #[uniform(0)]
    pub uniforms: LaserUniforms,
}

impl Material for LaserMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(LASER_LINE_SHADER_HANDLE)
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct ReticleUniforms {
    pub color: LinearRgba,
    pub time: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ReticleMaterial {
    #[uniform(0)]
    pub uniforms: ReticleUniforms,
}

impl Material for ReticleMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(RETICLE_SHADER_HANDLE)
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy_mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if let Some(depth_stencil) = &mut descriptor.depth_stencil {
            depth_stencil.depth_compare = bevy::render::render_resource::CompareFunction::Always;
            depth_stencil.depth_write_enabled = false;
        }
        Ok(())
    }
}


/// Tag on the targeting reticle quad entity.
#[derive(Component)]
pub struct TargetingReticle;

#[derive(Resource)]
pub struct RetroEffectsAsset {
    pub font: Handle<Font>,
}

/// Pre-built material handles for each tier.
#[derive(Resource)]
pub struct TierMaterials {
    pub normal: [Handle<SphereMaterial>; 9],
    pub core: [Handle<StandardMaterial>; 9],
}

/// Pre-built mesh handles for each tier.
#[derive(Resource)]
pub struct TierMeshes {
    pub outer: [Handle<Mesh>; 9],
    pub core: [Handle<Mesh>; 9],
}

/// Build all 9 tier materials.
fn build_tier_materials(
    materials: &mut Assets<SphereMaterial>,
    std_materials: &mut Assets<StandardMaterial>,
) -> TierMaterials {
    let normal = std::array::from_fn(|i| {
        let color = TIER_COLORS[i];
        let base_color = LinearRgba::from(color) * 0.05;
        let line_color = LinearRgba::from(color) * 1.5;
        materials.add(SphereMaterial {
            uniforms: SphereUniforms {
                color: line_color,
                base_color,
            },
        })
    });
    let core = std::array::from_fn(|i| {
        let color = TIER_COLORS[i];
        std_materials.add(StandardMaterial {
            base_color: color,
            emissive: LinearRgba::from(color) * 2.0,
            ..default()
        })
    });
    TierMaterials { normal, core }
}

/// Return the correct material handle for a tier (1-indexed).
pub fn material_for_tier(tier: u8, tier_mats: &TierMaterials) -> Handle<SphereMaterial> {
    let idx = (tier as usize).saturating_sub(1).min(8);
    tier_mats.normal[idx].clone()
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

fn setup_visuals_shaders(mut shaders: ResMut<Assets<Shader>>) {
    let _ = shaders.insert(
        &SPHERE_GRID_SHADER_HANDLE,
        Shader::from_wgsl(
            include_str!("../assets/shaders/sphere_grid.wgsl"),
            "shaders/sphere_grid.wgsl",
        ),
    );
    let _ = shaders.insert(
        &FLOOR_GRID_SHADER_HANDLE,
        Shader::from_wgsl(
            include_str!("../assets/shaders/floor_grid.wgsl"),
            "shaders/floor_grid.wgsl",
        ),
    );
    let _ = shaders.insert(
        &LASER_LINE_SHADER_HANDLE,
        Shader::from_wgsl(
            include_str!("../assets/shaders/laser_line.wgsl"),
            "shaders/laser_line.wgsl",
        ),
    );
    let _ = shaders.insert(
        &RETICLE_SHADER_HANDLE,
        Shader::from_wgsl(
            include_str!("../assets/shaders/reticle.wgsl"),
            "shaders/reticle.wgsl",
        ),
    );

}

pub struct VisualPlugin;

impl Plugin for VisualPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SphereMaterial>::default())
            .add_plugins(MaterialPlugin::<FloorMaterial>::default())
            .add_plugins(MaterialPlugin::<LaserMaterial>::default())
            .add_plugins(MaterialPlugin::<ReticleMaterial>::default())
            .add_systems(PreStartup, setup_visuals_shaders)
            // Systems
            .add_systems(Startup, setup_visuals)
            .add_systems(
                Update,
                (
                    update_preview_material,
                    update_preview_label.after(crate::launcher::update_launcher_preview_visuals),
                    update_labels_3d_position,
                    handle_keyboard_toggles,
                    update_aim_guide_line,
                    update_targeting_reticle.after(crate::launcher::update_launcher_preview_visuals),
                    animate_merged_spawns,
                    animate_fulfilling_spheres,
                    cleanup_orphaned_labels,
                    handle_placeholder_bursts,
                    update_matrix_particles,
                    update_sphere_effects,
                ),
            )
            .add_systems(
                OnExit(crate::game_state::AppState::InGame),
                cleanup_launcher_visuals,
            )
            // Observer: fires whenever a Sphere component is added to any entity
            .add_observer(on_sphere_added);
    }
}

// ---------------------------------------------------------------------------
// Startup — arena geometry + preview sphere
// ---------------------------------------------------------------------------

fn setup_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sphere_materials: ResMut<Assets<SphereMaterial>>,
    mut floor_materials: ResMut<Assets<FloorMaterial>>,
    mut laser_materials: ResMut<Assets<LaserMaterial>>,
    mut reticle_materials: ResMut<Assets<ReticleMaterial>>,
    settings: Res<GameSettings>,
    asset_server: Res<AssetServer>,
) {
    // Build and store tier materials
    let tier_mats = build_tier_materials(&mut sphere_materials, &mut materials);
    commands.insert_resource(tier_mats);

    // Build and store tier meshes
    let mut outer_meshes = Vec::new();
    let mut core_meshes = Vec::new();
    for tier in 1..=9 {
        let radius = get_radius(tier);
        outer_meshes.push(meshes.add(
            bevy::math::primitives::Sphere::new(radius)
                .mesh()
                .uv(32, 18),
        ));
        core_meshes.push(meshes.add(
            bevy::math::primitives::Sphere::new(radius * 0.65)
                .mesh()
                .uv(16, 8),
        ));
    }
    let tier_meshes = TierMeshes {
        outer: outer_meshes.try_into().unwrap(),
        core: core_meshes.try_into().unwrap(),
    };
    commands.insert_resource(tier_meshes);

    // Store Retro font handle
    let font_handle = asset_server.load("fonts/ShareTechMono-Regular.ttf");
    commands.insert_resource(RetroEffectsAsset {
        font: font_handle.clone(),
    });

    let half_w = settings.arena_width * 0.5;
    let depth = settings.arena_depth;
    let wh = settings.wall_height;

    // Center Z of the floor and walls relative to launcher_z so it extends back
    let center_z = settings.launcher_z - depth * 0.5;

    // ---- Floor ----
    let floor_mat = floor_materials.add(FloorMaterial {
        uniforms: FloorUniforms {
            grid_color: LinearRgba::from(Color::hsl(120.0, 0.50, 0.28)), // Muted ghostly phosphor green
            bg_color: LinearRgba::from(Color::BLACK), // Transparent/black background
        },
    });
    commands.spawn((
        Name::new("Arena Floor"),
        Mesh3d(meshes.add(Cuboid::new(settings.arena_width, 0.1, depth))),
        MeshMaterial3d(floor_mat.clone()),
        Transform::from_xyz(0.0, -0.05, center_z),
        Collider::cuboid(settings.arena_width * 0.5, 0.05, depth * 0.5),
    ));

    // ---- Side Diagnostics Decks ----
    // Spawn left side deck (raised to connect flush with the top of the left wall)
    commands.spawn((
        Name::new("Side Deck Left"),
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.1, depth))),
        MeshMaterial3d(floor_mat.clone()),
        Transform::from_xyz(-half_w - 0.1 - 4.0, wh - 0.05, center_z),
    ));

    // Spawn right side deck (raised to connect flush with the top of the right wall)
    commands.spawn((
        Name::new("Side Deck Right"),
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.1, depth))),
        MeshMaterial3d(floor_mat.clone()),
        Transform::from_xyz(half_w + 0.1 + 4.0, wh - 0.05, center_z),
    ));

    // ---- Left wall ----
    commands.spawn((
        Name::new("Wall Left"),
        Mesh3d(meshes.add(Cuboid::new(0.2, wh, depth))),
        MeshMaterial3d(floor_mat.clone()),
        Transform::from_xyz(-half_w - 0.1, wh * 0.5, center_z),
        Collider::cuboid(0.1, wh * 0.5, depth * 0.5),
    ));

    // ---- Right wall ----
    commands.spawn((
        Name::new("Wall Right"),
        Mesh3d(meshes.add(Cuboid::new(0.2, wh, depth))),
        MeshMaterial3d(floor_mat.clone()),
        Transform::from_xyz(half_w + 0.1, wh * 0.5, center_z),
        Collider::cuboid(0.1, wh * 0.5, depth * 0.5),
    ));

    // ---- Back wall ----
    commands.spawn((
        Name::new("Wall Back"),
        Mesh3d(meshes.add(Cuboid::new(settings.arena_width + 0.4, wh, 0.2))),
        MeshMaterial3d(floor_mat.clone()),
        Transform::from_xyz(0.0, wh * 0.5, settings.launcher_z - depth - 0.1),
        Collider::cuboid(settings.arena_width * 0.5 + 0.2, wh * 0.5, 0.1),
    ));

    // ---- Aim guide line (laser pointer) ----
    let laser_mat = laser_materials.add(LaserMaterial {
        uniforms: LaserUniforms {
            color: LinearRgba::from(TIER_COLORS[0]),
            time: 0.0,
            _padding1: 0.0,
            _padding2: 0.0,
            _padding3: 0.0,
        },
    });
    commands.spawn((
        Name::new("Aim Guide Line"),
        AimGuideLine,
        Mesh3d(meshes.add(Cuboid::new(0.25, 0.001, depth))),
        MeshMaterial3d(laser_mat),
        Transform::from_xyz(0.0, 0.003, center_z),
        Visibility::Hidden,
    ));

    // ---- Targeting reticle flat ring system ----
    let reticle_mat = reticle_materials.add(ReticleMaterial {
        uniforms: ReticleUniforms {
            color: LinearRgba::from(TIER_COLORS[0]),
            time: 0.0,
            _padding1: 0.0,
            _padding2: 0.0,
            _padding3: 0.0,
        },
    });
    commands.spawn((
        Name::new("Targeting Reticle"),
        TargetingReticle,
        Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial3d(reticle_mat),
        Transform::from_xyz(0.0, 0.002, settings.launcher_z).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Visibility::Hidden,
    ));

    // ---- Launcher preview sphere ----
    let preview_entity = commands
        .spawn((
            Name::new("Launcher Preview"),
            LauncherPreview,
            Mesh3d(meshes.add(bevy::math::primitives::Sphere::new(1.0).mesh().uv(32, 18))),
            MeshMaterial3d(sphere_materials.add(SphereMaterial {
                uniforms: SphereUniforms {
                    color: LinearRgba::from(TIER_COLORS[0]) * 1.5,
                    base_color: LinearRgba::from(TIER_COLORS[0]) * 0.05,
                },
            })),
            Transform::from_xyz(0.0, 0.0, settings.launcher_z),
            Visibility::Visible,
        ))
        .id();

    // Spawn nested emissive core for preview sphere
    let core_mesh = meshes.add(bevy::math::primitives::Sphere::new(0.65).mesh().uv(16, 8));
    let core_mat = materials.add(StandardMaterial {
        base_color: TIER_COLORS[0],
        emissive: LinearRgba::from(TIER_COLORS[0]) * 2.0,
        ..default()
    });
    let core_entity = commands
        .spawn((
            SphereCore,
            PreviewCore,
            Mesh3d(core_mesh),
            MeshMaterial3d(core_mat),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();
    commands.entity(preview_entity).add_child(core_entity);

    // ---- Preview label ----
    let init_radius = get_radius(1);
    commands.spawn((
        PreviewLabel,
        Text2d::new("1"),
        TextFont {
            font: font_handle.clone(),
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::WHITE),
        TextLayout::new_with_justify(Justify::Center),
        bevy::text::LineHeight::Px(22.0),
        Transform::from_xyz(0.0, init_radius + 0.35, settings.launcher_z + 0.15)
            .with_scale(Vec3::splat(0.015)),
        Visibility::Hidden,
    ));
}

// ---------------------------------------------------------------------------
// Observer — attach visual mesh and spawn root-level 2D label
// ---------------------------------------------------------------------------

fn on_sphere_added(
    trigger: On<Add, Sphere>,
    sphere_query: Query<&Sphere>,
    tier_mats: Option<Res<TierMaterials>>,
    tier_meshes: Option<Res<TierMeshes>>,
    effects_asset: Option<Res<RetroEffectsAsset>>,
    mut commands: Commands,
) {
    let entity = trigger.event_target();
    let Ok(sphere) = sphere_query.get(entity) else {
        return;
    };
    let (Some(tier_mats), Some(tier_meshes)) = (tier_mats, tier_meshes) else {
        return;
    };
    let idx = (sphere.tier as usize).saturating_sub(1).min(8);
    let mat = material_for_tier(sphere.tier, &tier_mats);
    let mesh = tier_meshes.outer[idx].clone();

    // Spawn 3D visual mesh child entity centered at parent, and add Visibility component on parent
    commands
        .entity(entity)
        .insert(Visibility::default())
        .with_children(|parent| {
            // 1. Outer sphere shell using the custom grid shader
            parent.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                SphereVisual,
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));

            // 2. Nested emissive core
            let core_mesh = tier_meshes.core[idx].clone();
            let core_mat = tier_mats.core[idx].clone();
            parent.spawn((
                SphereCore,
                Mesh3d(core_mesh),
                MeshMaterial3d(core_mat),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
        });

    // Spawn a root-level 3D text label that will follow the sphere in 3D space
    let mut label_cmd = commands.spawn((
        BillboardLabel(entity),
        Text2d::new(sphere.tier.to_string()),
        TextColor(Color::WHITE),
        TextLayout::new_with_justify(Justify::Center),
        bevy::text::LineHeight::Px(22.0),
        Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(0.015)),
        Visibility::Hidden,
    ));

    if let Some(ref asset) = effects_asset {
        label_cmd.insert(TextFont {
            font: asset.font.clone(),
            font_size: 22.0,
            ..default()
        });
    } else {
        label_cmd.insert(TextFont {
            font_size: 22.0,
            ..default()
        });
    }
}

// ---------------------------------------------------------------------------
// Update — keep preview sphere material in sync with current queue tier
// ---------------------------------------------------------------------------

fn update_preview_material(
    queue: Option<Res<DispenserQueue>>,
    tier_mats: Option<Res<TierMaterials>>,
    mut preview_query: Query<&mut MeshMaterial3d<SphereMaterial>, With<LauncherPreview>>,
    mut preview_core_query: Query<&mut MeshMaterial3d<StandardMaterial>, With<PreviewCore>>,
) {
    let (Some(queue), Some(tier_mats)) = (queue, tier_mats) else {
        return;
    };

    let Ok(mut mat_handle) = preview_query.single_mut() else {
        return;
    };
    *mat_handle = MeshMaterial3d(material_for_tier(queue.current, &tier_mats));

    if let Some(mut core_mat_handle) = preview_core_query.iter_mut().next() {
        let idx = (queue.current as usize).saturating_sub(1).min(8);
        let core_mat = tier_mats.core[idx].clone();
        if core_mat_handle.0 != core_mat {
            core_mat_handle.0 = core_mat;
        }
    }
}

// Position each active label in 3D space above the sphere surface facing the camera.
fn update_labels_3d_position(
    colorblind: Res<ColorblindMode>,
    sphere_query: Query<(Entity, &Transform, &Sphere)>,
    mut label_query: Query<(&BillboardLabel, &mut Transform, &mut Visibility), Without<Sphere>>,
    camera_3d_query: Query<&GlobalTransform, With<Camera3d>>,
) {
    if !colorblind.0 {
        // If colorblind mode is OFF, hide all labels and exit
        for (_, _, mut visibility) in label_query.iter_mut() {
            crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
        }
        return;
    }

    let Ok(cam_transform) = camera_3d_query.single() else {
        return;
    };
    let cam_pos = cam_transform.translation();
    let cam_rotation = cam_transform.compute_transform().rotation;

    for (label, mut label_transform, mut visibility) in label_query.iter_mut() {
        if let Ok((_, sphere_transform, sphere)) = sphere_query.get(label.0) {
            let sphere_pos = sphere_transform.translation;
            if sphere_pos.y < -0.1 {
                crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
                continue;
            }
            let radius = get_radius(sphere.tier);

            // Compute direction to camera to project the label directly on the sphere's surface facing the camera
            let dir_to_cam = (cam_pos - sphere_pos).normalize();
            label_transform.translation = sphere_pos + dir_to_cam * (radius + 0.03);
            label_transform.rotation = cam_rotation;
            label_transform.scale = Vec3::splat(0.015);

            crate::utils::set_visibility(&mut visibility, Visibility::Inherited);
        } else {
            crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
        }
    }
}

// Track and position the launcher preview label in 3D space above the launcher preview sphere facing the camera.
fn update_preview_label(
    queue: Option<Res<DispenserQueue>>,
    colorblind: Option<Res<ColorblindMode>>,
    launcher_state: Res<LauncherState>,
    settings: Res<GameSettings>,
    camera_3d_query: Query<&GlobalTransform, With<Camera3d>>,
    mut label_query: Query<
        (&mut Text2d, &mut Transform, &mut Visibility),
        With<PreviewLabel>,
    >,
) {
    let (Some(queue), Some(colorblind)) = (queue, colorblind) else {
        return;
    };
    let Ok((mut text, mut label_transform, mut visibility)) = label_query.single_mut() else {
        return;
    };

    // Update text
    let new_text = queue.current.to_string();
    if text.0 != new_text {
        text.0 = new_text;
    }

    // Toggle visibility based on colorblind mode AND the preview sphere's visibility
    let elapsed = launcher_state.cooldown_timer.elapsed_secs();
    let is_preview_visible = launcher_state.cooldown_timer.is_finished() || elapsed >= 0.4;

    if colorblind.0 && is_preview_visible {
        let Ok(cam_transform) = camera_3d_query.single() else {
            return;
        };
        let cam_pos = cam_transform.translation();
        let cam_rotation = cam_transform.compute_transform().rotation;

        let radius = get_radius(queue.current);
        let world_pos = Vec3::new(launcher_state.active_x, radius, settings.launcher_z);

        // Position directly on the surface facing the camera
        let dir_to_cam = (cam_pos - world_pos).normalize();
        label_transform.translation = world_pos + dir_to_cam * (radius + 0.03);
        label_transform.rotation = cam_rotation;
        label_transform.scale = Vec3::splat(0.015);

        crate::utils::set_visibility(&mut visibility, Visibility::Inherited);
    } else {
        crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
    }
}

// Despawn label entities whose sphere has already been despawned.
fn cleanup_orphaned_labels(
    mut commands: Commands,
    label_query: Query<(Entity, &BillboardLabel)>,
    sphere_query: Query<Entity, With<Sphere>>,
) {
    for (label_entity, label) in label_query.iter() {
        if sphere_query.get(label.0).is_err() {
            commands.entity(label_entity).despawn();
        }
    }
}

// Toggle colorblind mode (Key C) and aiming guide (Key V) via keyboard shortcuts.
fn handle_keyboard_toggles(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut colorblind: ResMut<ColorblindMode>,
    mut aim_line_mode: ResMut<AimLineMode>,
) {
    if keyboard.just_pressed(KeyCode::KeyC) {
        colorblind.0 = !colorblind.0;
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        aim_line_mode.0 = !aim_line_mode.0;
    }
}

fn update_aim_guide_line(
    aim_line_mode: Res<AimLineMode>,
    launcher_state: Res<LauncherState>,
    dispenser_queue: Option<Res<DispenserQueue>>,
    time: Res<Time>,
    state: Option<Res<State<crate::game_state::AppState>>>,
    mut query: Query<(&mut Transform, &mut Visibility, &MeshMaterial3d<LaserMaterial>), With<AimGuideLine>>,
    mut laser_materials: ResMut<Assets<LaserMaterial>>,
) {
    if let Ok((mut transform, mut visibility, mat_handle)) = query.single_mut() {
        let is_in_game = state
            .map(|s| *s.get() == crate::game_state::AppState::InGame)
            .unwrap_or(false);
        if aim_line_mode.0 && is_in_game {
            crate::utils::set_visibility(&mut visibility, Visibility::Visible);
            transform.translation.x = launcher_state.active_x;
            
            if let Some(mat) = laser_materials.get_mut(&mat_handle.0) {
                let current_tier = dispenser_queue.as_ref().map(|dq| dq.current).unwrap_or(1);
                let color = TIER_COLORS[(current_tier as usize).saturating_sub(1).min(8)];
                mat.uniforms.color = LinearRgba::from(color);
                mat.uniforms.time = time.elapsed_secs();
            }
        } else {
            crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
        }
    }
}

fn update_targeting_reticle(
    launcher_state: Res<LauncherState>,
    settings: Res<GameSettings>,
    dispenser_queue: Option<Res<DispenserQueue>>,
    time: Res<Time>,
    state: Option<Res<State<crate::game_state::AppState>>>,
    mut query: Query<(&mut Transform, &mut Visibility, &MeshMaterial3d<ReticleMaterial>), With<TargetingReticle>>,
    mut reticle_materials: ResMut<Assets<ReticleMaterial>>,
    preview_query: Query<&Transform, (With<LauncherPreview>, Without<TargetingReticle>)>,
) {
    if let Ok((mut transform, mut visibility, mat_handle)) = query.single_mut() {
        let is_in_game = state
            .map(|s| *s.get() == crate::game_state::AppState::InGame)
            .unwrap_or(false);
        
        if is_in_game {
            let elapsed = launcher_state.cooldown_timer.elapsed_secs();
            let is_preview_visible = launcher_state.cooldown_timer.is_finished() || elapsed >= 0.4;
            
            if is_preview_visible {
                crate::utils::set_visibility(&mut visibility, Visibility::Visible);
                
                let current_tier = dispenser_queue.as_ref().map(|dq| dq.current).unwrap_or(1);
                let radius = crate::core_math::get_radius(current_tier);
                
                // Align reticle position perfectly with the preview sphere center
                if let Ok(preview_transform) = preview_query.single() {
                    transform.translation = preview_transform.translation;
                } else {
                    transform.translation.x = launcher_state.active_x;
                    transform.translation.y = radius;
                    transform.translation.z = settings.launcher_z;
                }
                
                // Scale reticle to frame the sphere nicely
                let reticle_scale = radius * 4.2;
                transform.scale = Vec3::splat(reticle_scale);
                
                if let Some(mat) = reticle_materials.get_mut(&mat_handle.0) {
                    let color = TIER_COLORS[(current_tier as usize).saturating_sub(1).min(8)];
                    mat.uniforms.color = LinearRgba::from(color);
                    mat.uniforms.time = time.elapsed_secs();
                }
            } else {
                crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
            }
        } else {
            crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
        }
    }
}

pub fn compute_merge_scale(elapsed: f32, duration: f32) -> f32 {
    let t = (elapsed / duration).clamp(0.0, 1.0);
    // Spring elastic bounce: starts at 0.5, expands quickly to 1.25, and settles back to 1.0
    if t < 0.3 {
        let nt = t / 0.3;
        0.5 + nt * 0.75 // 0.5 -> 1.25
    } else if t < 0.6 {
        let nt = (t - 0.3) / 0.3;
        1.25 - nt * 0.30 // 1.25 -> 0.95
    } else {
        let nt = (t - 0.6) / 0.4;
        0.95 + nt * 0.05 // 0.95 -> 1.0
    }
}

pub fn compute_fulfillment_scale(elapsed: f32, duration: f32) -> f32 {
    let t = (elapsed / duration).clamp(0.0, 1.0);
    if t < 0.75 {
        1.0
    } else {
        let t_pop = (t - 0.75) / 0.25; // 0.0 .. 1.0
        if t_pop < 0.4 {
            let factor = t_pop / 0.4;
            1.0 + factor * 0.15
        } else {
            let factor = (t_pop - 0.4) / 0.6;
            1.15 * (1.0 - factor)
        }
    }
}

pub fn animate_merged_spawns(
    cooldown_query: Query<&crate::physics::MergeCooldown>,
    fulfilling_query: Query<&crate::game_state::Fulfilling>,
    mut visual_query: Query<(&ChildOf, &mut Transform), Or<(With<SphereVisual>, With<SphereCore>)>>,
    mut label_query: Query<
        (&BillboardLabel, &mut Transform),
        (Without<SphereVisual>, Without<SphereCore>),
    >,
) {
    for (child_of, mut transform) in visual_query.iter_mut() {
        let parent = child_of.0;
        if fulfilling_query.contains(parent) {
            continue; // Skip fulfilling spheres
        }
        if let Ok(cooldown) = cooldown_query.get(parent) {
            let scale = compute_merge_scale(
                cooldown.timer.elapsed_secs(),
                cooldown.timer.duration().as_secs_f32(),
            );
            transform.scale = Vec3::splat(scale);
        } else {
            if transform.scale != Vec3::ONE {
                transform.scale = Vec3::ONE;
            }
        }
    }
    for (label, mut transform) in label_query.iter_mut() {
        let parent = label.0;
        if fulfilling_query.contains(parent) {
            continue; // Skip fulfilling spheres
        }
        if let Ok(cooldown) = cooldown_query.get(parent) {
            let scale = compute_merge_scale(
                cooldown.timer.elapsed_secs(),
                cooldown.timer.duration().as_secs_f32(),
            );
            transform.scale = Vec3::splat(scale);
        } else {
            if transform.scale != Vec3::ONE {
                transform.scale = Vec3::ONE;
            }
        }
    }
}

pub fn animate_fulfilling_spheres(
    fulfillment: Option<Res<crate::game_state::ActiveFulfillment>>,
    mut visual_query: Query<(&ChildOf, &mut Transform), Or<(With<SphereVisual>, With<SphereCore>)>>,
    mut label_query: Query<
        (&BillboardLabel, &mut Transform),
        (Without<SphereVisual>, Without<SphereCore>),
    >,
) {
    let Some(fulfillment) = fulfillment else {
        return;
    };
    if let Some(fulfilling_entity) = fulfillment.entity {
        let scale = compute_fulfillment_scale(
            fulfillment.timer.elapsed_secs(),
            fulfillment.timer.duration().as_secs_f32(),
        );
        let scale_vec = Vec3::splat(scale);

        for (child_of, mut transform) in visual_query.iter_mut() {
            if child_of.0 == fulfilling_entity {
                transform.scale = scale_vec;
            }
        }
        for (label, mut transform) in label_query.iter_mut() {
            if label.0 == fulfilling_entity {
                transform.scale = scale_vec;
            }
        }
    }
}

pub fn cleanup_launcher_visuals(
    mut preview_query: Query<&mut Visibility, With<LauncherPreview>>,
    mut label_query: Query<&mut Visibility, (With<PreviewLabel>, Without<LauncherPreview>)>,
    mut line_query: Query<
        &mut Visibility,
        (
            With<AimGuideLine>,
            Without<LauncherPreview>,
            Without<PreviewLabel>,
        ),
    >,
) {
    if let Ok(mut vis) = preview_query.single_mut() {
        crate::utils::set_visibility(&mut vis, Visibility::Hidden);
    }
    if let Ok(mut vis) = label_query.single_mut() {
        crate::utils::set_visibility(&mut vis, Visibility::Hidden);
    }
    if let Ok(mut vis) = line_query.single_mut() {
        crate::utils::set_visibility(&mut vis, Visibility::Hidden);
    }
}

fn update_sphere_effects(
    effects_mode: Option<Res<crate::game_state::VisualEffectsMode>>,
    mut core_query: Query<&mut Visibility, With<SphereCore>>,
) {
    let is_effects_on = effects_mode
        .map(|m| *m == crate::game_state::VisualEffectsMode::On)
        .unwrap_or(true);

    let target_vis = if is_effects_on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    for mut vis in core_query.iter_mut() {
        crate::utils::set_visibility(&mut vis, target_vis);
    }
}

fn update_matrix_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particle_query: Query<(
        Entity,
        &mut Transform,
        &mut MatrixParticle,
        &mut TextColor,
        &mut Visibility,
    )>,
) {
    for (entity, mut transform, mut particle, mut text_color, mut visibility) in
        particle_query.iter_mut()
    {
        particle.lifetime.tick(time.delta());
        if particle.lifetime.is_finished() {
            commands.entity(entity).despawn();
        } else {
            // Make particle visible after layout is computed (avoids flashing at origin)
            if *visibility == Visibility::Hidden {
                *visibility = Visibility::Inherited;
            }
            // Apply velocity
            transform.translation += particle.velocity * time.delta_secs();

            // Fade alpha based on remaining lifetime
            let t = particle.lifetime.fraction();
            let alpha = (1.0 - t).clamp(0.0, 1.0);

            if let Color::LinearRgba(ref mut rgba) = text_color.0 {
                *rgba = LinearRgba::new(rgba.red, rgba.green, rgba.blue, alpha);
            } else {
                let to_rgba = text_color.0.to_linear();
                text_color.0 = Color::LinearRgba(LinearRgba::new(
                    to_rgba.red,
                    to_rgba.green,
                    to_rgba.blue,
                    alpha,
                ));
            }
        }
    }
}

pub fn handle_placeholder_bursts(
    mut commands: Commands,
    effects_mode: Option<Res<crate::game_state::VisualEffectsMode>>,
    effects_asset: Option<Res<RetroEffectsAsset>>,
    mut merge_events: MessageReader<crate::game_state::MergeBurstEvent>,
    mut fulfill_events: MessageReader<crate::game_state::FulfillmentBurstEvent>,
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
) {
    let is_effects_on = effects_mode
        .map(|m| *m == crate::game_state::VisualEffectsMode::On)
        .unwrap_or(true);

    let Some(effects_asset) = effects_asset else {
        return;
    };

    let cam_rotation = camera_query
        .iter()
        .next()
        .map(|t| t.compute_transform().rotation)
        .unwrap_or(Quat::IDENTITY);

    // Read merge events
    for event in merge_events.read() {
        if is_effects_on {
            #[cfg(target_arch = "wasm32")]
            let num_particles = (5 + (event.tier as usize)).min(15);
            #[cfg(not(target_arch = "wasm32"))]
            let num_particles = 10 + (event.tier as usize) * 2;
            for _ in 0..num_particles {
                let offset = Vec3::new(
                    rand::random_range(-0.5..0.5),
                    rand::random_range(-0.5..0.5),
                    rand::random_range(-0.5..0.5),
                );
                let pos = event.position + offset;

                let vel = Vec3::new(
                    rand::random_range(-0.5..0.5),
                    rand::random_range(-2.5..-1.0),
                    rand::random_range(-0.5..0.5),
                );

                let chars = ['0', '1', '#', '@', '$', '%', '&', 'X', 'Y', '*'];
                let char_idx = rand::random_range(0..chars.len());
                let char_str = chars[char_idx].to_string();

                let font_size = rand::random_range(16.0..28.0);
                let lifetime = rand::random_range(0.8..1.5);

                commands.spawn((
                    Text2d::new(char_str),
                    TextFont {
                        font: effects_asset.font.clone(),
                        font_size,
                        ..default()
                    },
                    TextColor(Color::hsl(120.0, 0.95, 0.6)),
                    Transform::from_translation(pos).with_rotation(cam_rotation),
                    Visibility::Hidden,
                    MatrixParticle {
                        velocity: vel,
                        lifetime: Timer::from_seconds(lifetime, TimerMode::Once),
                    },
                ));
            }
        }
    }

    // Read fulfillment events
    for event in fulfill_events.read() {
        if is_effects_on {
            #[cfg(target_arch = "wasm32")]
            let num_particles = (12 + (event.tier as usize) * 2).min(25);
            #[cfg(not(target_arch = "wasm32"))]
            let num_particles = 25 + (event.tier as usize) * 3;
            for _ in 0..num_particles {
                let offset = Vec3::new(
                    rand::random_range(-0.8..0.8),
                    rand::random_range(-0.8..0.8),
                    rand::random_range(-0.8..0.8),
                );
                let pos = event.position + offset;

                let vel = Vec3::new(
                    rand::random_range(-0.8..0.8),
                    rand::random_range(-3.5..-1.5),
                    rand::random_range(-0.8..0.8),
                );

                let chars = ['0', '1', '#', '@', '$', '%', '&', 'X', 'Y', '*'];
                let char_idx = rand::random_range(0..chars.len());
                let char_str = chars[char_idx].to_string();

                let font_size = rand::random_range(18.0..32.0);
                let lifetime = rand::random_range(1.0..2.0);

                commands.spawn((
                    Text2d::new(char_str),
                    TextFont {
                        font: effects_asset.font.clone(),
                        font_size,
                        ..default()
                    },
                    TextColor(Color::hsl(120.0, 0.95, 0.65)),
                    Transform::from_translation(pos).with_rotation(cam_rotation),
                    Visibility::Hidden,
                    MatrixParticle {
                        velocity: vel,
                        lifetime: Timer::from_seconds(lifetime, TimerMode::Once),
                    },
                ));
            }
        }
    }
}

