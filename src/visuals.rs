use crate::core_math::get_radius;
use crate::game_state::{AimLineMode, ColorblindMode, DispenserQueue, GameSettings, Sphere};
use crate::launcher::{LauncherPreview, LauncherState};
use bevy::prelude::*;
use bevy_rapier3d::prelude::Collider;

// ---------------------------------------------------------------------------
// Tier color palette — warm perceptual gradient, violet → sky blue → lime →
// orange → gold, designed to be distinct across 10 steps.
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
// Resources
// ---------------------------------------------------------------------------

/// Pre-built material handles for each tier.
#[derive(Resource)]
pub struct TierMaterials {
    pub normal: [Handle<StandardMaterial>; 9],
}

/// Build all 9 tier materials.
fn build_tier_materials(materials: &mut Assets<StandardMaterial>) -> TierMaterials {
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

/// Return the correct material handle for a tier (1-indexed).
pub fn material_for_tier(
    tier: u8,
    _colorblind: bool,
    tier_mats: &TierMaterials,
) -> Handle<StandardMaterial> {
    let idx = (tier as usize).saturating_sub(1).min(8);
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
                    update_preview_label,
                    update_labels_screen_position,
                    handle_keyboard_toggles,
                    update_aim_guide_line,
                    animate_merged_spawns,
                    animate_fulfilling_spheres,
                    cleanup_orphaned_labels,
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
    let depth = settings.arena_depth;
    let wh = settings.wall_height;

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
        Collider::cuboid(settings.arena_width * 0.5, 0.05, depth * 0.5),
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
    commands.spawn((
        Name::new("Wall Back"),
        Mesh3d(meshes.add(Cuboid::new(settings.arena_width + 0.4, wh, 0.2))),
        MeshMaterial3d(wall_mat),
        Transform::from_xyz(0.0, wh * 0.5, settings.launcher_z - depth - 0.1),
        Collider::cuboid(settings.arena_width * 0.5 + 0.2, wh * 0.5, 0.1),
    ));

    // ---- Aim guide line ----
    let guide_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.8, 0.0, 0.15), // Faint green
        perceptual_roughness: 1.0,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("Aim Guide Line"),
        AimGuideLine,
        Mesh3d(meshes.add(Cuboid::new(0.02, 0.005, depth))),
        MeshMaterial3d(guide_mat),
        Transform::from_xyz(0.0, 0.005, center_z),
        Visibility::Hidden,
    ));

    // ---- Launcher preview sphere ----
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

    // ---- Preview label ----
    commands.spawn((
        PreviewLabel,
        Text2d::new("1"),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::WHITE),
        TextLayout::new_with_justify(Justify::Center),
        bevy::text::LineHeight::Px(22.0),
        Transform::from_xyz(0.0, 0.0, 10.0),
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
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let entity = trigger.event_target();
    let Ok(sphere) = sphere_query.get(entity) else {
        return;
    };
    let Some(tier_mats) = tier_mats else {
        return;
    };
    let radius = get_radius(sphere.tier);
    let mat = material_for_tier(sphere.tier, false, &tier_mats);
    let mesh = meshes.add(
        bevy::math::primitives::Sphere::new(radius)
            .mesh()
            .uv(32, 18),
    );

    // Spawn 3D visual mesh child entity offset by radius in Y
    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            SphereVisual,
            Transform::from_xyz(0.0, radius, 0.0),
        ));
    });

    // Spawn a root-level 2D text label that will be screen-projected on top of 3D
    commands.spawn((
        BillboardLabel(entity),
        Text2d::new(sphere.tier.to_string()),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::WHITE),
        TextLayout::new_with_justify(Justify::Center),
        bevy::text::LineHeight::Px(22.0),
        Transform::from_xyz(0.0, 0.0, 10.0), // Z coordinate in 2D space
        Visibility::Hidden,
    ));
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
    let (Some(queue), Some(tier_mats)) = (queue, tier_mats) else {
        return;
    };
    let cb = colorblind.map(|r| r.0).unwrap_or(false);

    let Ok(mut mat_handle) = preview_query.single_mut() else {
        return;
    };
    *mat_handle = MeshMaterial3d(material_for_tier(queue.current, cb, &tier_mats));
}

// Project each label's tracked sphere from 3D world space to 2D screen space.
fn update_labels_screen_position(
    colorblind: Res<ColorblindMode>,
    sphere_query: Query<(Entity, &Transform, &Sphere)>,
    mut label_query: Query<(&BillboardLabel, &mut Transform, &mut Visibility), Without<Sphere>>,
    camera_3d_query: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    window_query: Query<&Window>,
) {
    if !colorblind.0 {
        // If colorblind mode is OFF, hide all labels and exit
        for (_, _, mut visibility) in label_query.iter_mut() {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
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
    let cam_pos = cam_transform.translation();

    for (label, mut label_transform, mut visibility) in label_query.iter_mut() {
        if let Ok((sphere_entity, sphere_transform, sphere)) = sphere_query.get(label.0) {
            let sphere_pos = sphere_transform.translation;
            if sphere_pos.y < -0.1 {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
                continue;
            }
            let radius = get_radius(sphere.tier);

            // Project a point at the visual center of the sphere in world space
            let world_pos = sphere_pos + Vec3::Y * radius;

            // Analytical ray-sphere occlusion check: is the visual sphere occluded from the camera's perspective?
            let mut occluded = false;
            let to_target = world_pos - cam_pos;
            let target_dist = to_target.length();
            if target_dist > 0.0 {
                let ray_dir = to_target / target_dist;

                for (other_entity, other_transform, other_sphere) in sphere_query.iter() {
                    if other_entity == sphere_entity {
                        continue;
                    }
                    let other_radius = get_radius(other_sphere.tier);
                    // The visual center of the other sphere
                    let other_visual_center = other_transform.translation + Vec3::Y * other_radius;

                    let v = other_visual_center - cam_pos;
                    let t = v.dot(ray_dir);
                    // Only check spheres that lie between the camera and our target sphere (with a small buffer)
                    if t > 0.0 && t < target_dist - radius * 0.1 {
                        let d2 = v.length_squared() - t * t;
                        let r2 = other_radius * other_radius;
                        if d2 < r2 {
                            occluded = true;
                            break;
                        }
                    }
                }
            }

            if occluded {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
                continue;
            }

            // Project coordinates
            if let Some(ndc) = camera.world_to_ndc(cam_transform, world_pos) {
                if ndc.z < 0.0 || ndc.z > 1.0 {
                    if *visibility != Visibility::Hidden {
                        *visibility = Visibility::Hidden;
                    }
                    continue;
                }
                // NDC [-1,1] → screen pixels (origin at center for Camera2d)
                let screen_x = ndc.x * win_w * 0.5;
                let screen_y = ndc.y * win_h * 0.5;
                label_transform.translation.x = screen_x;
                label_transform.translation.y = screen_y;

                if *visibility != Visibility::Visible {
                    *visibility = Visibility::Visible;
                }
            } else {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}

// Track and project the launcher preview label onto the launcher preview sphere.
fn update_preview_label(
    queue: Option<Res<DispenserQueue>>,
    colorblind: Option<Res<ColorblindMode>>,
    camera_3d_query: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    preview_query: Query<(&Transform, &Visibility), With<LauncherPreview>>,
    mut label_query: Query<
        (&mut Text2d, &mut Transform, &mut Visibility),
        (With<PreviewLabel>, Without<LauncherPreview>),
    >,
    window_query: Query<&Window>,
) {
    let (Some(queue), Some(colorblind)) = (queue, colorblind) else {
        return;
    };
    let Ok((preview_transform, preview_visibility)) = preview_query.single() else {
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
    let is_preview_visible =
        *preview_visibility == Visibility::Visible || *preview_visibility == Visibility::Inherited;

    if colorblind.0 && is_preview_visible {
        let Ok((camera, cam_transform)) = camera_3d_query.single() else {
            return;
        };
        let Ok(window) = window_query.single() else {
            return;
        };
        let win_w = window.width();
        let win_h = window.height();

        let world_pos = preview_transform.translation;
        if let Some(ndc) = camera.world_to_ndc(cam_transform, world_pos) {
            if ndc.z < 0.0 || ndc.z > 1.0 {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            } else {
                let screen_x = ndc.x * win_w * 0.5;
                let screen_y = ndc.y * win_h * 0.5;
                label_transform.translation.x = screen_x;
                label_transform.translation.y = screen_y;

                if *visibility != Visibility::Visible {
                    *visibility = Visibility::Visible;
                }
            }
        } else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
        }
    } else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
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
    state: Option<Res<State<crate::game_state::AppState>>>,
    mut query: Query<(&mut Transform, &mut Visibility), With<AimGuideLine>>,
) {
    if let Ok((mut transform, mut visibility)) = query.single_mut() {
        let is_in_game = state.map(|s| *s.get() == crate::game_state::AppState::InGame).unwrap_or(false);
        if aim_line_mode.0 && is_in_game {
            if *visibility != Visibility::Visible {
                *visibility = Visibility::Visible;
            }
            transform.translation.x = launcher_state.active_x;
        } else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

pub fn animate_merged_spawns(
    cooldown_query: Query<&crate::physics::MergeCooldown>,
    fulfilling_query: Query<&crate::game_state::Fulfilling>,
    mut visual_query: Query<(&ChildOf, &mut Transform), With<SphereVisual>>,
    mut label_query: Query<(&BillboardLabel, &mut Transform), Without<SphereVisual>>,
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
    for (label, mut transform) in label_query.iter_mut() {
        let parent = label.0;
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
}

pub fn animate_fulfilling_spheres(
    fulfillment: Option<Res<crate::game_state::ActiveFulfillment>>,
    mut visual_query: Query<(&ChildOf, &mut Transform), With<SphereVisual>>,
    mut label_query: Query<(&BillboardLabel, &mut Transform), Without<SphereVisual>>,
) {
    let Some(fulfillment) = fulfillment else {
        return;
    };
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
        for (label, mut transform) in label_query.iter_mut() {
            if label.0 == fulfilling_entity {
                transform.scale = Vec3::splat(scale);
            }
        }
    }
}
