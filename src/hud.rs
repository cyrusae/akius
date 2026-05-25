use bevy::prelude::*;
use crate::game_state::{Score, ActiveOrder, DispenserQueue, ColorblindMode};
use crate::visuals::TIER_COLORS;

#[derive(Component)]
pub struct ScoreText;

#[derive(Component)]
pub struct OrderText;

#[derive(Component)]
pub struct NextSpherePreviewText;

#[derive(Component)]
pub struct NextSpherePreviewSwatch;

#[derive(Component)]
pub struct ColorblindButton;

#[derive(Component)]
pub struct ColorblindButtonText;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, setup_hud)
            .add_systems(
                Update,
                (
                    update_score_hud,
                    update_order_hud,
                    update_next_sphere_hud,
                    handle_colorblind_button,
                    update_colorblind_button_text,
                ),
            );
    }
}

fn setup_hud(
    mut commands: Commands,
) {
    // Main full screen container
    commands.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        position_type: PositionType::Absolute,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::SpaceBetween,
        padding: UiRect::all(Val::Px(20.0)),
        ..default()
    }).with_children(|parent| {
        // Top row
        parent.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        }).with_children(|top_row| {
            // Score panel (Top-left)
            top_row.spawn((
                Node {
                    padding: UiRect::new(Val::Px(15.0), Val::Px(15.0), Val::Px(10.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                BorderColor::all(Color::WHITE),
            )).with_children(|score_panel| {
                score_panel.spawn((
                    ScoreText,
                    Text::new("Score: 0"),
                    TextFont {
                        font_size: 24.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });

            // Target Order panel (Top-center)
            top_row.spawn((
                Node {
                    padding: UiRect::new(Val::Px(15.0), Val::Px(15.0), Val::Px(10.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                BorderColor::all(Color::WHITE),
            )).with_children(|order_panel| {
                order_panel.spawn((
                    OrderText,
                    Text::new("Target: Tier 6"),
                    TextFont {
                        font_size: 24.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });

            // Next sphere preview panel (Top-right)
            top_row.spawn((
                Node {
                    padding: UiRect::new(Val::Px(15.0), Val::Px(15.0), Val::Px(10.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                BorderColor::all(Color::WHITE),
            )).with_children(|next_panel| {
                next_panel.spawn((
                    Text::new("Next:"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                
                // Color swatch
                next_panel.spawn((
                    NextSpherePreviewSwatch,
                    Node {
                        width: Val::Px(24.0),
                        height: Val::Px(24.0),
                        border_radius: BorderRadius::all(Val::Px(12.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                )).with_children(|swatch| {
                    swatch.spawn((
                        NextSpherePreviewText,
                        Text::new(""),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });
        });

        // Bottom row
        parent.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            align_items: AlignItems::Center,
            ..default()
        }).with_children(|bottom_row| {
            // Colorblind button (Bottom-right)
            bottom_row.spawn((
                ColorblindButton,
                Button,
                Node {
                    width: Val::Px(160.0),
                    height: Val::Px(45.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BorderColor::all(Color::WHITE),
                BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
            )).with_children(|btn| {
                btn.spawn((
                    ColorblindButtonText,
                    Text::new("Colorblind: OFF"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });
    });
}

fn update_score_hud(
    score: Option<Res<Score>>,
    mut query: Query<&mut Text, With<ScoreText>>,
) {
    let Some(score) = score else { return; };
    let Ok(mut text) = query.single_mut() else { return; };
    let new_text = format!("Score: {}", score.total);
    if text.0 != new_text {
        text.0 = new_text;
    }
}

fn update_order_hud(
    active_order: Option<Res<ActiveOrder>>,
    mut query: Query<&mut Text, With<OrderText>>,
) {
    let Some(active_order) = active_order else { return; };
    let Ok(mut text) = query.single_mut() else { return; };
    let new_text = format!("Target: Tier {}", active_order.target_tier);
    if text.0 != new_text {
        text.0 = new_text;
    }
}

fn update_next_sphere_hud(
    queue: Option<Res<DispenserQueue>>,
    mut swatch_query: Query<&mut BackgroundColor, With<NextSpherePreviewSwatch>>,
    mut text_query: Query<&mut Text, With<NextSpherePreviewText>>,
) {
    let Some(queue) = queue else { return; };
    let idx = (queue.next as usize).saturating_sub(1).min(9);

    let color = TIER_COLORS[idx];

    if let Ok(mut bg) = swatch_query.single_mut() {
        if bg.0 != color {
            bg.0 = color;
        }
    }
    if let Ok(mut text) = text_query.single_mut() {
        let new_text = queue.next.to_string();
        if text.0 != new_text {
            text.0 = new_text;
        }
    }
}

fn handle_colorblind_button(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<ColorblindButton>),
    >,
    mut colorblind: ResMut<ColorblindMode>,
) {
    for (interaction, mut bg_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                colorblind.0 = !colorblind.0;
                *bg_color = BackgroundColor(Color::srgb(0.35, 0.35, 0.35));
            }
            Interaction::Hovered => {
                *bg_color = BackgroundColor(Color::srgb(0.25, 0.25, 0.25));
            }
            Interaction::None => {
                *bg_color = BackgroundColor(Color::srgb(0.15, 0.15, 0.15));
            }
        }
    }
}

fn update_colorblind_button_text(
    colorblind: Res<ColorblindMode>,
    mut query: Query<&mut Text, With<ColorblindButtonText>>,
) {
    if colorblind.is_changed() {
        if let Ok(mut text) = query.single_mut() {
            text.0 = format!("Colorblind: {}", if colorblind.0 { "ON" } else { "OFF" });
        }
    }
}
