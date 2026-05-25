use bevy::prelude::*;
use bevy_rapier3d::prelude::Collider;
use crate::game_state::{ColorblindMode, DispenserQueue, GameSettings, Sphere};
use crate::core_math::get_radius;
use crate::launcher::LauncherPreview;

// ---------------------------------------------------------------------------
// Tier color palette — warm perceptual gradient, violet → sky blue → lime →
// orange → gold, designed to be distinct across 13 steps.
// ---------------------------------------------------------------------------
pub const TIER_COLORS: [Color; 13] = [
    Color::hsl(270.0, 0.70, 0.55), // Tier  1 — violet
    Color::hsl(240.0, 0.65, 0.60), // Tier  2 — indigo-blue
    Color::hsl(210.0, 0.70, 0.58), // Tier  3 — sky blue
    Color::hsl(180.0, 0.65, 0.50), // Tier  4 — cyan
    Color::hsl(150.0, 0.60, 0.48), // Tier  5 — teal-green
    Color::hsl(120.0, 0.60, 0.45), // Tier  6 — lime green
    Color::hsl( 90.0, 0.62, 0.48), // Tier  7 — yellow-green
    Color::hsl( 60.0, 0.72, 0.50), // Tier  8 — yellow
    Color::hsl( 40.0, 0.80, 0.53), // Tier  9 — amber
    Color::hsl( 25.0, 0.85, 0.52), // Tier 10 — orange
    Color::hsl( 10.0, 0.80, 0.52), // Tier 11 — red-orange
    Color::hsl(  0.0, 0.75, 0.52), // Tier 12 — red
    Color::hsl( 45.0, 0.90, 0.55), // Tier 13 — gold (max)
];

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Tag on the mesh child entity attached to a physics `Sphere` entity.
#[derive(Component)]
pub struct SphereVisual;

/// Tag on the billboard 3D/2D text child entity of a `Sphere` entity.
#[derive(Component)]
pub struct BillboardLabel;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Pre-built material handles for each tier.
#[derive(Resource)]
pub struct TierMaterials {
    pub normal: [Handle<StandardMaterial>; 13],
}

/// Build all 13 tier materials.
fn build_tier_materials(
    materials: &mut Assets<StandardMaterial>,
) -> TierMaterials {
    let normal = std::array::from_fn(|i| {
        let color = TIER_COLORS[i];
        materials.add(StandardMaterial {
            base_color: color,
            emissive: LinearRgba::from(color) * 0.15,
            perceptual_roughness: 0.4,
            metallic: 0.1,
            ..default()
        })
    });
    TierMaterials { normal }
}

/// Return the correct material handle for a tier (1-indexed) given the current
/// colorblind mode.
pub fn material_for_tier(
    tier: u8,
    _colorblind: bool,
    tier_mats: &TierMaterials,
) -> Handle<StandardMaterial> {
    let idx = (tier as usize).saturating_sub(1).min(12);
    tier_mats.normal[idx].clone()
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct VisualPlugin;

impl Plugin for VisualPlugin {
    fn build(&self, app: &mut App) {
        app
            // Resources
            .init_resource::<ColorblindMode>()
            // Systems
            .add_systems(Startup, setup_visuals)
            .add_systems(
                Update,
                (
                    update_preview_material,
                    update_billboards,
                    handle_keyboard_colorblind_toggle,
                    update_billboard_visibility,
                    animate_merged_spawns,
                    animate_fulfilling_spheres,
                ),
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
    settings: Res<GameSettings>,
) {
    // Build and store tier materials
    let tier_mats = build_tier_materials(&mut materials);
    commands.insert_resource(tier_mats);

    let half_w = settings.arena_width * 0.5;
    let depth  = settings.arena_depth;
    let wh     = settings.wall_height;

    // Center Z of the floor and walls relative to launcher_z so it extends back
    let center_z = settings.launcher_z - depth * 0.5;

    // ---- Floor ----
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::hsl(220.0, 0.12, 0.18),
        perceptual_roughness: 0.85,
        ..default()
    });
    commands.spawn((
        Name::new("Arena Floor"),
        Mesh3d(meshes.add(Cuboid::new(settings.arena_width, 0.1, depth))),
        MeshMaterial3d(floor_mat),
        Transform::from_xyz(0.0, -0.05, center_z),
    ));

    // ---- Left wall ----
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::hsl(220.0, 0.15, 0.25),
        perceptual_roughness: 0.8,
        ..default()
    });
    commands.spawn((
        Name::new("Wall Left"),
        Mesh3d(meshes.add(Cuboid::new(0.2, wh, depth))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_xyz(-half_w - 0.1, wh * 0.5, center_z),
        Collider::cuboid(0.1, wh * 0.5, depth * 0.5),
    ));

    // ---- Right wall ----
    commands.spawn((
        Name::new("Wall Right"),
        Mesh3d(meshes.add(Cuboid::new(0.2, wh, depth))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_xyz(half_w + 0.1, wh * 0.5, center_z),
        Collider::cuboid(0.1, wh * 0.5, depth * 0.5),
    ));

    // ---- Back wall ----
    // Located at the far end of the floor (settings.launcher_z - depth)
    commands.spawn((
        Name::new("Wall Back"),
        Mesh3d(meshes.add(Cuboid::new(settings.arena_width + 0.4, wh, 0.2))),
        MeshMaterial3d(wall_mat),
        Transform::from_xyz(0.0, wh * 0.5, settings.launcher_z - depth - 0.1),
        Collider::cuboid(settings.arena_width * 0.5 + 0.2, wh * 0.5, 0.1),
    ));

    // ---- Dashed loss-boundary line ----
    // A row of small flat cubes across the arena width at loss_boundary_z.
    let dash_mat = materials.add(StandardMaterial {
        base_color: Color::hsl(0.0, 0.85, 0.55),
        emissive: LinearRgba::new(0.6, 0.05, 0.05, 1.0),
        ..default()
    });
    let dash_w   = 0.3_f32;
    let dash_gap = 0.2_f32;
    let dash_h   = 0.04_f32;
    let dash_d   = 0.08_f32;
    let step     = dash_w + dash_gap;
    let n_dashes = ((settings.arena_width) / step).floor() as i32;
    let start_x  = -(n_dashes as f32 * step * 0.5) + dash_w * 0.5;

    for i in 0..n_dashes {
        let x = start_x + i as f32 * step;
        commands.spawn((
            Name::new(format!("LossDash{i}")),
            Mesh3d(meshes.add(Cuboid::new(dash_w, dash_h, dash_d))),
            MeshMaterial3d(dash_mat.clone()),
            Transform::from_xyz(x, dash_h * 0.5, settings.loss_boundary_z),
        ));
    }

    // ---- Launcher preview sphere ----
    // Starts with tier-1 material; updated each frame by update_preview_material.
    // Mesh is created with unit size (1.0) so it scales perfectly to true sphere size.
    commands.spawn((
        Name::new("Launcher Preview"),
        LauncherPreview,
        Mesh3d(meshes.add(bevy::math::primitives::Sphere::new(1.0).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: TIER_COLORS[0],
            emissive: LinearRgba::from(TIER_COLORS[0]) * 0.3,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, settings.launcher_z),
        Visibility::Visible,
    ));
}

// ---------------------------------------------------------------------------
// Observer — attach mesh + material when a Sphere component is added
// ---------------------------------------------------------------------------

fn on_sphere_added(
    trigger: On<Add, Sphere>,
    sphere_query: Query<&Sphere>,
    tier_mats: Option<Res<TierMaterials>>,
    colorblind: Option<Res<ColorblindMode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let entity = trigger.event_target();
    let Ok(sphere) = sphere_query.get(entity) else { return; };
    let Some(tier_mats) = tier_mats else { return; };
    let cb = colorblind.map(|r| r.0).unwrap_or(false);
    let radius = get_radius(sphere.tier);
    let mat    = material_for_tier(sphere.tier, cb, &tier_mats);
    let mesh   = meshes.add(bevy::math::primitives::Sphere::new(radius).mesh().uv(32, 18));
    let initial_visibility = if cb {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    // Spawn 3D visual mesh child entity offset by radius in Y
    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            SphereVisual,
            Transform::from_xyz(0.0, radius, 0.0),
        ));

        // Spawn 2D billboard text child entity above the sphere
        parent.spawn((
            BillboardLabel,
            Text2d::new(sphere.tier.to_string()),
            TextFont {
                font_size: 40.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(0.0, radius * 2.0 + 0.15, 0.0)
                .with_scale(Vec3::splat(0.025)),
            initial_visibility,
        ));
    });
}

// ---------------------------------------------------------------------------
// Update — keep preview sphere material in sync with current queue tier
// ---------------------------------------------------------------------------

fn update_preview_material(
    queue: Option<Res<DispenserQueue>>,
    tier_mats: Option<Res<TierMaterials>>,
    colorblind: Option<Res<ColorblindMode>>,
    mut preview_query: Query<&mut MeshMaterial3d<StandardMaterial>, With<LauncherPreview>>,
) {
    let (Some(queue), Some(tier_mats)) = (queue, tier_mats) else { return; };
    let cb = colorblind.map(|r| r.0).unwrap_or(false);

    let Ok(mut mat_handle) = preview_query.single_mut() else { return; };
    *mat_handle = MeshMaterial3d(material_for_tier(queue.current, cb, &tier_mats));
}

// Rotate all billboard labels to face the camera.
fn update_billboards(
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
    mut billboard_query: Query<&mut Transform, With<BillboardLabel>>,
) {
    let Ok(camera_global_transform) = camera_query.single() else { return; };
    let camera_rotation = camera_global_transform.compute_transform().rotation;
    for mut transform in billboard_query.iter_mut() {
        transform.rotation = camera_rotation;
    }
}

// Toggle colorblind mode via the Keyboard C key.
fn handle_keyboard_colorblind_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut colorblind: ResMut<ColorblindMode>,
) {
    if keyboard.just_pressed(KeyCode::KeyC) {
        colorblind.0 = !colorblind.0;
    }
}


pub fn animate_merged_spawns(
    cooldown_query: Query<&crate::physics::MergeCooldown>,
    fulfilling_query: Query<&crate::game_state::Fulfilling>,
    mut visual_query: Query<(&ChildOf, &mut Transform), With<SphereVisual>>,
    mut billboard_query: Query<(&ChildOf, &mut Transform), (With<BillboardLabel>, Without<SphereVisual>)>,
) {
    for (child_of, mut transform) in visual_query.iter_mut() {
        let parent = child_of.0;
        if fulfilling_query.contains(parent) {
            continue; // Skip fulfilling spheres
        }
        if let Ok(cooldown) = cooldown_query.get(parent) {
            let elapsed = cooldown.timer.elapsed_secs();
            let duration = cooldown.timer.duration().as_secs_f32();
            let t = (elapsed / duration).clamp(0.0, 1.0);
            let scale = 0.8 + t * 0.2;
            transform.scale = Vec3::splat(scale);
        } else {
            if transform.scale != Vec3::ONE {
                transform.scale = Vec3::ONE;
            }
        }
    }
    for (child_of, mut transform) in billboard_query.iter_mut() {
        let parent = child_of.0;
        if fulfilling_query.contains(parent) {
            continue; // Skip fulfilling spheres
        }
        if let Ok(cooldown) = cooldown_query.get(parent) {
            let elapsed = cooldown.timer.elapsed_secs();
            let duration = cooldown.timer.duration().as_secs_f32();
            let t = (elapsed / duration).clamp(0.0, 1.0);
            let scale = 0.8 + t * 0.2;
            transform.scale = Vec3::splat(0.025 * scale);
        } else {
            if transform.scale != Vec3::splat(0.025) {
                transform.scale = Vec3::splat(0.025);
            }
        }
    }
}

pub fn animate_fulfilling_spheres(
    fulfillment: Option<Res<crate::game_state::ActiveFulfillment>>,
    mut visual_query: Query<(&ChildOf, &mut Transform), With<SphereVisual>>,
    mut billboard_query: Query<(&ChildOf, &mut Transform), (With<BillboardLabel>, Without<SphereVisual>)>,
) {
    let Some(fulfillment) = fulfillment else { return; };
    if let Some(fulfilling_entity) = fulfillment.entity {
        let elapsed = fulfillment.timer.elapsed_secs();
        let duration = fulfillment.timer.duration().as_secs_f32();
        let t = (elapsed / duration).clamp(0.0, 1.0);
        
        let scale = if t < 0.75 {
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
        };
        
        for (child_of, mut transform) in visual_query.iter_mut() {
            if child_of.0 == fulfilling_entity {
                transform.scale = Vec3::splat(scale);
            }
        }
        for (child_of, mut transform) in billboard_query.iter_mut() {
            if child_of.0 == fulfilling_entity {
                transform.scale = Vec3::splat(0.025 * scale);
            }
        }
    }
}

pub fn update_billboard_visibility(
    colorblind: Res<ColorblindMode>,
    mut query: Query<&mut Visibility, With<BillboardLabel>>,
) {
    if colorblind.is_changed() {
        let new_visibility = if colorblind.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        for mut visibility in query.iter_mut() {
            *visibility = new_visibility;
        }
    }
}
