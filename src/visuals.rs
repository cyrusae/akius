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

/// Converts a 1-indexed tier into a clamped index into the 9-element tier arrays
/// (`TIER_COLORS`, `TierMaterials`, `TierMeshes`).
pub fn tier_index(tier: u8) -> usize {
    (tier as usize).saturating_sub(1).min(8)
}

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
    /// Full-size (width, height) of the particle quad; shrunk over the lifetime
    /// as a stand-in for alpha fade so all particles can share one material.
    pub base_size: Vec2,
}

/// Shared mesh/material handles for Matrix burst particles, so spawning bursts
/// never creates new assets.
#[derive(Resource)]
pub struct MatrixParticleAssets {
    pub mesh: Handle<Mesh>,
    pub merge_material: Handle<StandardMaterial>,
    pub fulfill_material: Handle<StandardMaterial>,
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
    tier_mats.normal[tier_index(tier)].clone()
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
                    update_labels_screen_position,
                    handle_keyboard_toggles,
                    update_aim_guide_line,
                    update_targeting_reticle
                        .after(crate::launcher::update_launcher_preview_visuals),
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
        outer_meshes.push(
            meshes.add(
                bevy::math::primitives::Sphere::new(radius)
                    .mesh()
                    .uv(32, 18),
            ),
        );
        core_meshes.push(
            meshes.add(
                bevy::math::primitives::Sphere::new(radius * 0.65)
                    .mesh()
                    .uv(16, 8),
            ),
        );
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

    // Shared assets for Matrix burst particles (unit quad, scaled per particle).
    // Additive blending gives the phosphor-glow look on the black arena without
    // needing per-particle alpha (and thus per-particle materials).
    commands.insert_resource(MatrixParticleAssets {
        mesh: meshes.add(Rectangle::new(1.0, 1.0)),
        merge_material: materials.add(StandardMaterial {
            base_color: Color::hsl(120.0, 0.95, 0.6),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
        fulfill_material: materials.add(StandardMaterial {
            base_color: Color::hsl(120.0, 0.95, 0.65),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
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
        },
    });
    commands.spawn((
        Name::new("Targeting Reticle"),
        TargetingReticle,
        Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial3d(reticle_mat),
        Transform::from_xyz(0.0, 0.002, settings.launcher_z)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
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
    commands.spawn((
        PreviewLabel,
        Text::new("1"),
        TextFont {
            font: font_handle.clone(),
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Visibility::Hidden,
    ));
}

// ---------------------------------------------------------------------------
// Observer — attach visual mesh and spawn root-level 2D label
// ---------------------------------------------------------------------------

pub(crate) fn on_sphere_added(
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
    let idx = tier_index(sphere.tier);
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

    // Spawn a root-level UI text label that will be screen-projected on top of 3D
    let mut label_cmd = commands.spawn((
        BillboardLabel(entity),
        Text::new(sphere.tier.to_string()),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
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

/// Maps an undistorted viewport position to where the CRT post-process shader
/// actually displays it on screen.
///
/// The shader's `curve()` runs in the *inverse* direction: for each output pixel
/// it computes which source-texture position to sample (`textureSample(src, curve(out_uv))`).
/// A scene point projected to source position `p` therefore appears on screen at
/// `q` where `curve(q) = p` — i.e. pulled *toward* the screen center, not pushed
/// away from it. Labels must apply the inverse of `curve`, which has a closed form
/// because the shader distorts X first and then derives Y from the distorted X:
///
/// ```text
/// forward (shader):  X = a*(1 + b^2/BEND);  Y = b*(1 + X^2/BEND)
/// inverse (here):    b = Y/(1 + X^2/BEND);  a = X/(1 + b^2/BEND)
/// ```
///
/// `BEND` must stay in sync with `bend` in `assets/shaders/crt_post_process.wgsl`.
pub fn crt_screen_position(viewport_pos: Vec2, window_size: Vec2) -> Vec2 {
    const BEND: f32 = 3.8;
    let s = viewport_pos / window_size - Vec2::splat(0.5);
    let qy = s.y / (1.0 + (s.x * s.x) / BEND);
    let qx = s.x / (1.0 + (qy * qy) / BEND);
    (Vec2::new(qx, qy) + Vec2::splat(0.5)) * window_size
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
    // Only reassign when the handle actually changes: an unconditional write dirties
    // Changed<MeshMaterial3d> every frame and forces the render world to re-bind the
    // preview's material even when the tier is unchanged.
    let new_mat = material_for_tier(queue.current, &tier_mats);
    if mat_handle.0 != new_mat {
        mat_handle.0 = new_mat;
    }

    if let Some(mut core_mat_handle) = preview_core_query.iter_mut().next() {
        let core_mat = tier_mats.core[tier_index(queue.current)].clone();
        if core_mat_handle.0 != core_mat {
            core_mat_handle.0 = core_mat;
        }
    }
}

// Position each active label in 2D screen space above its tracked 3D sphere, applying CRT barrel distortion if active.
fn update_labels_screen_position(
    colorblind: Res<ColorblindMode>,
    effects_mode: Option<Res<crate::game_state::VisualEffectsMode>>,
    sphere_query: Query<(Entity, &Transform, &Sphere)>,
    mut label_query: Query<
        (&BillboardLabel, &mut Node, &mut Visibility, &ComputedNode),
        Without<Sphere>,
    >,
    camera_3d_query: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    window_query: Query<&Window>,
) {
    if !colorblind.0 {
        // If colorblind mode is OFF, hide all labels and exit
        for (_, _, mut visibility, _) in label_query.iter_mut() {
            crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
        }
        return;
    }

    let Ok((camera, cam_transform)) = camera_3d_query.single() else {
        return;
    };
    let Ok(window) = window_query.single() else {
        return;
    };
    let win_w = window.width();
    let win_h = window.height();
    let is_effects_on = effects_mode
        .map(|m| *m == crate::game_state::VisualEffectsMode::On)
        .unwrap_or(true);

    for (label, mut node, mut visibility, computed_node) in label_query.iter_mut() {
        if let Ok((_, sphere_transform, _)) = sphere_query.get(label.0) {
            let sphere_pos = sphere_transform.translation;
            if sphere_pos.y < -0.1 {
                crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
                continue;
            }

            // Project coordinates
            if let Ok(viewport_pos) = camera.world_to_viewport(cam_transform, sphere_pos) {
                let pos = if is_effects_on {
                    // Match the CRT post-process: invert the shader's sampling curve
                    // to find where the sphere is actually drawn on screen.
                    crt_screen_position(viewport_pos, Vec2::new(win_w, win_h))
                } else {
                    viewport_pos
                };

                // Center UI text node (font size 22.0, single digit is approx 13x22px)
                // Use ComputedNode size if populated, otherwise fallback to original static offsets.
                let offset_x = if computed_node.size.x > 0.0 {
                    computed_node.size.x * 0.5
                } else {
                    6.5
                };
                let offset_y = if computed_node.size.y > 0.0 {
                    computed_node.size.y * 0.5
                } else {
                    11.0
                };

                node.position_type = PositionType::Absolute;
                node.left = Val::Px(pos.x - offset_x);
                node.top = Val::Px(pos.y - offset_y);

                crate::utils::set_visibility(&mut visibility, Visibility::Visible);
            } else {
                crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
            }
        } else {
            crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
        }
    }
}

// Track and position the launcher preview label in 2D screen space, applying CRT barrel distortion if active.
fn update_preview_label(
    queue: Option<Res<DispenserQueue>>,
    colorblind: Option<Res<ColorblindMode>>,
    effects_mode: Option<Res<crate::game_state::VisualEffectsMode>>,
    launcher_state: Res<LauncherState>,
    settings: Res<GameSettings>,
    camera_3d_query: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut label_query: Query<
        (&mut Text, &mut Node, &mut Visibility, &ComputedNode),
        With<PreviewLabel>,
    >,
    window_query: Query<&Window>,
) {
    let (Some(queue), Some(colorblind)) = (queue, colorblind) else {
        return;
    };
    let Ok((mut text, mut node, mut visibility, computed_node)) = label_query.single_mut() else {
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
        let Ok((camera, cam_transform)) = camera_3d_query.single() else {
            return;
        };
        let Ok(window) = window_query.single() else {
            return;
        };
        let win_w = window.width();
        let win_h = window.height();
        let is_effects_on = effects_mode
            .map(|m| *m == crate::game_state::VisualEffectsMode::On)
            .unwrap_or(true);

        let radius = get_radius(queue.current);
        let world_pos = Vec3::new(launcher_state.active_x, radius, settings.launcher_z);

        if let Ok(viewport_pos) = camera.world_to_viewport(cam_transform, world_pos) {
            let pos = if is_effects_on {
                // Match the CRT post-process: invert the shader's sampling curve
                // to find where the sphere is actually drawn on screen.
                crt_screen_position(viewport_pos, Vec2::new(win_w, win_h))
            } else {
                viewport_pos
            };

            let offset_x = if computed_node.size.x > 0.0 {
                computed_node.size.x * 0.5
            } else {
                6.5
            };
            let offset_y = if computed_node.size.y > 0.0 {
                computed_node.size.y * 0.5
            } else {
                11.0
            };

            node.position_type = PositionType::Absolute;
            node.left = Val::Px(pos.x - offset_x);
            node.top = Val::Px(pos.y - offset_y);

            crate::utils::set_visibility(&mut visibility, Visibility::Visible);
        } else {
            crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
        }
    } else {
        crate::utils::set_visibility(&mut visibility, Visibility::Hidden);
    }
}

// Despawn label entities whose sphere has already been despawned.
pub(crate) fn cleanup_orphaned_labels(
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
    state: Option<Res<State<crate::game_state::AppState>>>,
    mut query: Query<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<LaserMaterial>,
        ),
        With<AimGuideLine>,
    >,
    mut laser_materials: ResMut<Assets<LaserMaterial>>,
    mut last_tier: Local<Option<u8>>,
) {
    if let Ok((mut transform, mut visibility, mat_handle)) = query.single_mut() {
        let is_in_game = state
            .map(|s| *s.get() == crate::game_state::AppState::InGame)
            .unwrap_or(false);
        if aim_line_mode.0 && is_in_game {
            crate::utils::set_visibility(&mut visibility, Visibility::Visible);
            transform.translation.x = launcher_state.active_x;

            // Only touch the material asset when the tier (and thus color) actually
            // changes. Mutating an asset fires AssetEvent::Modified, which forces the
            // render world to rebuild the material's GPU buffers — doing that every
            // frame is needless churn. The animation time comes from `globals.time`
            // inside the shader instead.
            let current_tier = dispenser_queue.as_ref().map(|dq| dq.current).unwrap_or(1);
            if *last_tier != Some(current_tier) {
                if let Some(mat) = laser_materials.get_mut(&mat_handle.0) {
                    let color = TIER_COLORS[tier_index(current_tier)];
                    mat.uniforms.color = LinearRgba::from(color);
                    *last_tier = Some(current_tier);
                }
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
    state: Option<Res<State<crate::game_state::AppState>>>,
    mut query: Query<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<ReticleMaterial>,
        ),
        With<TargetingReticle>,
    >,
    mut reticle_materials: ResMut<Assets<ReticleMaterial>>,
    preview_query: Query<&Transform, (With<LauncherPreview>, Without<TargetingReticle>)>,
    mut last_tier: Local<Option<u8>>,
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

                // Only mutate the material asset on tier changes (see update_aim_guide_line);
                // rotation/pulse animation is driven by `globals.time` in the shader.
                if *last_tier != Some(current_tier) {
                    if let Some(mat) = reticle_materials.get_mut(&mat_handle.0) {
                        let color = TIER_COLORS[tier_index(current_tier)];
                        mat.uniforms.color = LinearRgba::from(color);
                        *last_tier = Some(current_tier);
                    }
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
    // Labels are UI nodes: in Bevy 0.18 they carry `UiTransform`, not `Transform`.
    mut label_query: Query<(&BillboardLabel, &mut UiTransform)>,
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
    for (label, mut ui_transform) in label_query.iter_mut() {
        let parent = label.0;
        if fulfilling_query.contains(parent) {
            continue; // Skip fulfilling spheres
        }
        if let Ok(cooldown) = cooldown_query.get(parent) {
            let scale = compute_merge_scale(
                cooldown.timer.elapsed_secs(),
                cooldown.timer.duration().as_secs_f32(),
            );
            ui_transform.scale = Vec2::splat(scale);
        } else if ui_transform.scale != Vec2::ONE {
            ui_transform.scale = Vec2::ONE;
        }
    }
}

pub fn animate_fulfilling_spheres(
    fulfillment: Option<Res<crate::game_state::ActiveFulfillment>>,
    mut visual_query: Query<(&ChildOf, &mut Transform), Or<(With<SphereVisual>, With<SphereCore>)>>,
    // Labels are UI nodes: in Bevy 0.18 they carry `UiTransform`, not `Transform`.
    mut label_query: Query<(&BillboardLabel, &mut UiTransform)>,
) {
    let Some(fulfillment) = fulfillment else {
        return;
    };
    if let Some(fulfilling_entity) = fulfillment.entity {
        let scale = compute_fulfillment_scale(
            fulfillment.timer.elapsed_secs(),
            fulfillment.timer.duration().as_secs_f32(),
        );

        for (child_of, mut transform) in visual_query.iter_mut() {
            if child_of.0 == fulfilling_entity {
                transform.scale = Vec3::splat(scale);
            }
        }
        for (label, mut ui_transform) in label_query.iter_mut() {
            if label.0 == fulfilling_entity {
                ui_transform.scale = Vec2::splat(scale);
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

pub(crate) fn update_matrix_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particle_query: Query<(Entity, &mut Transform, &mut MatrixParticle)>,
) {
    for (entity, mut transform, mut particle) in particle_query.iter_mut() {
        particle.lifetime.tick(time.delta());
        if particle.lifetime.is_finished() {
            commands.entity(entity).despawn();
        } else {
            // Apply velocity
            transform.translation += particle.velocity * time.delta_secs();

            // Shrink toward zero over the lifetime (shared-material stand-in for
            // an alpha fade).
            let fade = (1.0 - particle.lifetime.fraction()).clamp(0.0, 1.0);
            let size = particle.base_size * fade;
            transform.scale = Vec3::new(size.x, size.y, 1.0);
        }
    }
}

/// Parameter ranges for one category of Matrix burst.
struct BurstParams {
    spread: f32,
    vel_xz: f32,
    vel_y: std::ops::Range<f32>,
    width: std::ops::Range<f32>,
    height: std::ops::Range<f32>,
    lifetime: std::ops::Range<f32>,
}

fn spawn_matrix_burst(
    commands: &mut Commands,
    assets: &MatrixParticleAssets,
    material: &Handle<StandardMaterial>,
    cam_rotation: Quat,
    position: Vec3,
    num_particles: usize,
    params: &BurstParams,
) {
    for _ in 0..num_particles {
        let offset = Vec3::new(
            rand::random_range(-params.spread..params.spread),
            rand::random_range(-params.spread..params.spread),
            rand::random_range(-params.spread..params.spread),
        );
        let vel = Vec3::new(
            rand::random_range(-params.vel_xz..params.vel_xz),
            rand::random_range(params.vel_y.clone()),
            rand::random_range(-params.vel_xz..params.vel_xz),
        );
        let size = Vec2::new(
            rand::random_range(params.width.clone()),
            rand::random_range(params.height.clone()),
        );
        let lifetime = rand::random_range(params.lifetime.clone());

        // Camera-facing unlit quads: 3D world-space particles. (These used to be
        // `Sprite`s, which stopped rendering when the 2D overlay camera was removed.)
        commands.spawn((
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(position + offset)
                .with_rotation(cam_rotation)
                .with_scale(Vec3::new(size.x, size.y, 1.0)),
            MatrixParticle {
                velocity: vel,
                lifetime: Timer::from_seconds(lifetime, TimerMode::Once),
                base_size: size,
            },
        ));
    }
}

pub fn handle_placeholder_bursts(
    mut commands: Commands,
    effects_mode: Option<Res<crate::game_state::VisualEffectsMode>>,
    particle_assets: Option<Res<MatrixParticleAssets>>,
    mut merge_events: MessageReader<crate::game_state::MergeBurstEvent>,
    mut fulfill_events: MessageReader<crate::game_state::FulfillmentBurstEvent>,
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
) {
    let is_effects_on = effects_mode
        .map(|m| *m == crate::game_state::VisualEffectsMode::On)
        .unwrap_or(true);
    let Some(particle_assets) = particle_assets else {
        return;
    };

    let cam_rotation = camera_query
        .iter()
        .next()
        .map(|t| t.compute_transform().rotation)
        .unwrap_or(Quat::IDENTITY);

    const MERGE_PARAMS: BurstParams = BurstParams {
        spread: 0.5,
        vel_xz: 0.5,
        vel_y: -2.5..-1.0,
        width: 0.03..0.06,
        height: 0.12..0.22,
        lifetime: 0.8..1.5,
    };
    const FULFILL_PARAMS: BurstParams = BurstParams {
        spread: 0.8,
        vel_xz: 0.8,
        vel_y: -3.5..-1.5,
        width: 0.04..0.08,
        height: 0.15..0.28,
        lifetime: 1.0..2.0,
    };

    // Read merge events
    for event in merge_events.read() {
        if is_effects_on {
            #[cfg(target_arch = "wasm32")]
            let num_particles = (5 + (event.tier as usize)).min(15);
            #[cfg(not(target_arch = "wasm32"))]
            let num_particles = 10 + (event.tier as usize) * 2;
            spawn_matrix_burst(
                &mut commands,
                &particle_assets,
                &particle_assets.merge_material,
                cam_rotation,
                event.position,
                num_particles,
                &MERGE_PARAMS,
            );
        }
    }

    // Read fulfillment events
    for event in fulfill_events.read() {
        if is_effects_on {
            #[cfg(target_arch = "wasm32")]
            let num_particles = (12 + (event.tier as usize) * 2).min(25);
            #[cfg(not(target_arch = "wasm32"))]
            let num_particles = 25 + (event.tier as usize) * 3;
            spawn_matrix_burst(
                &mut commands,
                &particle_assets,
                &particle_assets.fulfill_material,
                cam_rotation,
                event.position,
                num_particles,
                &FULFILL_PARAMS,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reimplements the forward `curve()` from `assets/shaders/crt_post_process.wgsl`
    /// (output pixel -> source sample position) for verification.
    fn shader_curve(uv: Vec2) -> Vec2 {
        let mut u = uv - Vec2::splat(0.5);
        let bend = Vec2::new(3.8, 3.8);
        u.x *= 1.0 + (u.y * u.y) / bend.x;
        u.y *= 1.0 + (u.x * u.x) / bend.y;
        u + Vec2::splat(0.5)
    }

    #[test]
    fn test_crt_screen_position_inverts_shader_curve() {
        // For any source position p, the label is placed at q = crt_screen_position(p).
        // The shader shows source point p at the output pixel q satisfying curve(q) = p,
        // so curve(q) must round-trip back to p exactly.
        let win = Vec2::new(800.0, 600.0);
        for &(x, y) in &[
            (400.0, 300.0), // center
            (100.0, 500.0), // bottom-left (launcher area)
            (700.0, 500.0), // bottom-right
            (50.0, 50.0),   // far corner
            (400.0, 550.0), // bottom-center
            (240.0, 480.0),
        ] {
            let p = Vec2::new(x, y);
            let q = crt_screen_position(p, win);
            let round_trip = shader_curve(q / win) * win;
            assert!(
                (round_trip - p).length() < 1e-3,
                "curve(crt_screen_position({p:?})) = {round_trip:?}, expected {p:?}"
            );
        }
    }

    #[test]
    fn test_crt_screen_position_pulls_toward_center() {
        // The CRT shader compresses the scene toward screen center, so the corrected
        // label position must sit between the raw projection and the center — never
        // outside it. (The original bug applied the curve forward, pushing labels
        // *away* from center, which made them drift off laterally with the ball.)
        let win = Vec2::new(800.0, 600.0);
        let center = win * 0.5;

        // A ball on the launcher line, left of center.
        let p = Vec2::new(200.0, 510.0);
        let q = crt_screen_position(p, win);
        assert!(
            q.x > p.x && q.x < center.x,
            "expected corrected x between raw ({}) and center ({}), got {}",
            p.x,
            center.x,
            q.x
        );

        // Dead-center stays put.
        let q_center = crt_screen_position(center, win);
        assert!((q_center - center).length() < 1e-4);
    }

    #[test]
    fn test_label_projection() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.insert_resource(GameSettings::default());
        app.insert_resource(ColorblindMode(true));
        app.insert_resource(LauncherState::default());
        app.insert_resource(DispenserQueue {
            current: 1,
            next: 2,
        });

        // Spawn 3D camera
        let cam_transform =
            Transform::from_xyz(0.0, 15.0, 18.0).looking_at(Vec3::new(0.0, 0.0, 5.0), Vec3::Y);
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
            cam_transform,
            GlobalTransform::from(cam_transform),
        ));

        // Spawn Window
        app.world_mut().spawn(Window {
            resolution: bevy::window::WindowResolution::new(800, 600),
            ..default()
        });

        // Spawn preview label as a UI node
        let preview_label_entity = app
            .world_mut()
            .spawn((
                PreviewLabel,
                Text::new("1"),
                TextFont::default(),
                TextColor(Color::WHITE),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ComputedNode::default(),
                Visibility::Hidden,
            ))
            .id();

        // Run systems: update_preview_label
        app.add_systems(Update, update_preview_label);

        // Run once
        app.update();

        // Check the label UI node properties
        let label_node = app
            .world()
            .entity(preview_label_entity)
            .get::<Node>()
            .unwrap();
        let label_vis = app
            .world()
            .entity(preview_label_entity)
            .get::<Visibility>()
            .unwrap();

        assert!(matches!(label_node.position_type, PositionType::Absolute));
        assert_eq!(*label_vis, Visibility::Hidden);
    }
}
