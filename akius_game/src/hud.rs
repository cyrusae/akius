use akius_core::{ActiveOrder, AimLineMode, ColorblindMode, DispenserQueue, Score};
use crate::visuals::TIER_COLORS;
use bevy::prelude::*;

#[derive(Component)]
pub struct CurrentScoreText;

#[derive(Component)]
pub struct HighScoreText;

#[derive(Component)]
pub struct HudTopRow;

#[derive(Component)]
pub struct HudBottomRow;

#[derive(Component)]
pub struct HudLeftCol;

#[derive(Component)]
pub struct HudCenterCol;

#[derive(Component)]
pub struct HudRightCol;

#[derive(Component)]
pub struct TargetSpherePreviewText;

#[derive(Component)]
pub struct TargetSpherePreviewSwatch;

#[derive(Component)]
pub struct NextSpherePreviewText;

#[derive(Component)]
pub struct NextSpherePreviewSwatch;

#[derive(Component)]
pub struct ColorblindButton;

#[derive(Component)]
pub struct ColorblindButtonText;

#[derive(Component)]
pub struct AimGuideButton;

#[derive(Component)]
pub struct AimGuideButtonText;

#[derive(Component)]
pub struct GameOverScreen;

#[derive(Component)]
pub struct RestartButton;

#[derive(Component)]
pub struct MainMenuScreen;

#[derive(Component)]
pub struct StartButton;

#[derive(Component)]
pub struct EffectsButton;

#[derive(Component)]
pub struct EffectsButtonText;

#[derive(Component)]
pub struct WinScreen;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_hud)
            .add_systems(
                Update,
                (
                    update_score_hud,
                    update_order_hud,
                    update_next_sphere_hud,
                    handle_colorblind_button,
                    update_colorblind_button_text,
                    handle_aim_guide_button,
                    update_aim_guide_button_text,
                    handle_effects_button,
                    update_effects_button_text,
                    responsive_hud_layout,
                ),
            )
            .add_systems(
                OnEnter(akius_core::AppState::MainMenu),
                spawn_main_menu_screen,
            )
            .add_systems(
                OnExit(akius_core::AppState::MainMenu),
                despawn_main_menu_screen,
            )
            .add_systems(
                Update,
                (handle_start_input, handle_start_button)
                    .run_if(in_state(akius_core::AppState::MainMenu)),
            )
            .add_systems(
                OnEnter(akius_core::AppState::GameOver),
                spawn_game_over_screen,
            )
            .add_systems(
                OnExit(akius_core::AppState::GameOver),
                despawn_game_over_screen,
            )
            .add_systems(OnEnter(akius_core::AppState::Win), spawn_win_screen)
            .add_systems(OnExit(akius_core::AppState::Win), despawn_win_screen)
            .add_systems(
                Update,
                (handle_restart_input, handle_restart_button).run_if(in_game_over_or_win_state),
            );
    }
}

fn setup_hud(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font_handle = asset_server.load("fonts/ShareTechMono-Regular.ttf");

    // Main full screen container
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(20.0)),
            ..default()
        })
        .with_children(|parent| {
            // Top row
            parent
                .spawn((
                    HudTopRow,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|top_row| {
                    // Left wrapper (33.333%)
                    top_row
                        .spawn((
                            HudLeftCol,
                            Node {
                                width: Val::Percent(33.333),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                        ))
                        .with_children(|left_col| {
                            // Score panel (Top-left)
                            left_col
                                .spawn((
                                    Node {
                                        padding: UiRect::new(
                                            Val::Px(15.0),
                                            Val::Px(15.0),
                                            Val::Px(10.0),
                                            Val::Px(10.0),
                                        ),
                                        border: UiRect::all(Val::Px(2.0)),
                                        flex_direction: FlexDirection::Row,
                                        flex_wrap: FlexWrap::Wrap,
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(20.0),
                                        row_gap: Val::Px(5.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                                    BorderColor::all(Color::WHITE),
                                ))
                                .with_children(|score_panel| {
                                    score_panel.spawn((
                                        CurrentScoreText,
                                        Text::new("Score: 000000"),
                                        TextFont {
                                            font: font_handle.clone(),
                                            font_size: 28.0,
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                    score_panel.spawn((
                                        HighScoreText,
                                        Text::new("HI: 000000"),
                                        TextFont {
                                            font: font_handle.clone(),
                                            font_size: 28.0,
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                });
                        });

                    // Center wrapper (33.333%)
                    top_row
                        .spawn((
                            HudCenterCol,
                            Node {
                                width: Val::Percent(33.333),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                        ))
                        .with_children(|center_col| {
                            // Target Order panel (Top-center)
                            center_col
                                .spawn((
                                    Node {
                                        padding: UiRect::new(
                                            Val::Px(15.0),
                                            Val::Px(15.0),
                                            Val::Px(10.0),
                                            Val::Px(10.0),
                                        ),
                                        border: UiRect::all(Val::Px(2.0)),
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(10.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                                    BorderColor::all(Color::WHITE),
                                ))
                                .with_children(|order_panel| {
                                    order_panel.spawn((
                                        Text::new("Target:"),
                                        TextFont {
                                            font: font_handle.clone(),
                                            font_size: 24.0,
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));

                                    // Color swatch
                                    order_panel
                                        .spawn((
                                            TargetSpherePreviewSwatch,
                                            Node {
                                                width: Val::Px(30.0),
                                                height: Val::Px(30.0),
                                                border_radius: BorderRadius::all(Val::Px(15.0)),
                                                justify_content: JustifyContent::Center,
                                                align_items: AlignItems::Center,
                                                ..default()
                                            },
                                            BackgroundColor(Color::WHITE),
                                        ))
                                        .with_children(|swatch| {
                                            swatch.spawn((
                                                TargetSpherePreviewText,
                                                Text::new(""),
                                                TextFont {
                                                    font: font_handle.clone(),
                                                    font_size: 20.0,
                                                    ..default()
                                                },
                                                TextColor(Color::WHITE),
                                            ));
                                        });
                                });
                        });

                    // Right wrapper (33.333%)
                    top_row
                        .spawn((
                            HudRightCol,
                            Node {
                                width: Val::Percent(33.333),
                                justify_content: JustifyContent::FlexEnd,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                        ))
                        .with_children(|right_col| {
                            // Next sphere preview panel (Top-right)
                            right_col
                                .spawn((
                                    Node {
                                        padding: UiRect::new(
                                            Val::Px(15.0),
                                            Val::Px(15.0),
                                            Val::Px(10.0),
                                            Val::Px(10.0),
                                        ),
                                        border: UiRect::all(Val::Px(2.0)),
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(10.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                                    BorderColor::all(Color::WHITE),
                                ))
                                .with_children(|next_panel| {
                                    next_panel.spawn((
                                        Text::new("Next:"),
                                        TextFont {
                                            font: font_handle.clone(),
                                            font_size: 24.0,
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));

                                    // Color swatch
                                    next_panel
                                        .spawn((
                                            NextSpherePreviewSwatch,
                                            Node {
                                                width: Val::Px(30.0),
                                                height: Val::Px(30.0),
                                                border_radius: BorderRadius::all(Val::Px(15.0)),
                                                justify_content: JustifyContent::Center,
                                                align_items: AlignItems::Center,
                                                ..default()
                                            },
                                            BackgroundColor(Color::WHITE),
                                        ))
                                        .with_children(|swatch| {
                                            swatch.spawn((
                                                NextSpherePreviewText,
                                                Text::new(""),
                                                TextFont {
                                                    font: font_handle.clone(),
                                                    font_size: 20.0,
                                                    ..default()
                                                },
                                                TextColor(Color::WHITE),
                                            ));
                                        });
                                });
                        });
                });

            // Bottom row
            parent
                .spawn((
                    HudBottomRow,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(15.0),
                        ..default()
                    },
                ))
                .with_children(|bottom_row| {
                    // Aim Guide button
                    bottom_row
                        .spawn((
                            AimGuideButton,
                            Button,
                            Node {
                                width: Val::Px(180.0),
                                height: Val::Px(50.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BorderColor::all(Color::WHITE),
                            BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                AimGuideButtonText,
                                Text::new("Aim Line: ON"),
                                TextFont {
                                    font: font_handle.clone(),
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });

                    // Visual Effects (FX) button
                    bottom_row
                        .spawn((
                            EffectsButton,
                            Button,
                            Node {
                                width: Val::Px(180.0),
                                height: Val::Px(50.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BorderColor::all(Color::WHITE),
                            BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                EffectsButtonText,
                                Text::new("FX: ON"),
                                TextFont {
                                    font: font_handle.clone(),
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });

                    // Colorblind button (Bottom-right)
                    bottom_row
                        .spawn((
                            ColorblindButton,
                            Button,
                            Node {
                                width: Val::Px(180.0),
                                height: Val::Px(50.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BorderColor::all(Color::WHITE),
                            BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                ColorblindButtonText,
                                Text::new("Numbers: ON"),
                                TextFont {
                                    font: font_handle.clone(),
                                    font_size: 18.0,
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
    high_score: Option<Res<crate::visuals::HighScore>>,
    mut current_query: Query<&mut Text, (With<CurrentScoreText>, Without<HighScoreText>)>,
    mut high_query: Query<&mut Text, (With<HighScoreText>, Without<CurrentScoreText>)>,
) {
    let Some(score) = score else {
        return;
    };
    let hi = high_score.map(|h| h.0).unwrap_or(0);

    if let Ok(mut text) = current_query.single_mut() {
        let new_text = format!("Score: {:06}", score.total);
        if text.0 != new_text {
            text.0 = new_text;
        }
    }

    if let Ok(mut text) = high_query.single_mut() {
        let new_text = format!("HI: {:06}", hi);
        if text.0 != new_text {
            text.0 = new_text;
        }
    }
}

fn update_order_hud(
    active_order: Option<Res<ActiveOrder>>,
    mut swatch_query: Query<&mut BackgroundColor, With<TargetSpherePreviewSwatch>>,
    mut text_query: Query<&mut Text, With<TargetSpherePreviewText>>,
) {
    let Some(active_order) = active_order else {
        return;
    };
    let color = TIER_COLORS[crate::visuals::tier_index(active_order.target_tier)];

    if let Ok(mut bg) = swatch_query.single_mut() {
        if bg.0 != color {
            bg.0 = color;
        }
    }
    if let Ok(mut text) = text_query.single_mut() {
        let new_text = active_order.target_tier.to_string();
        if text.0 != new_text {
            text.0 = new_text;
        }
    }
}

fn handle_effects_button(
    mut interaction_query: Query<&Interaction, (Changed<Interaction>, With<EffectsButton>)>,
    mut effects_mode: ResMut<crate::visuals::VisualEffectsMode>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    let mut toggle = false;
    for interaction in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            toggle = true;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyF) {
        toggle = true;
    }
    if toggle {
        *effects_mode = match *effects_mode {
            crate::visuals::VisualEffectsMode::On => crate::visuals::VisualEffectsMode::Off,
            crate::visuals::VisualEffectsMode::Off => crate::visuals::VisualEffectsMode::On,
        };
        info!("Visual effects toggled to: {:?}", *effects_mode);
    }
}

fn update_effects_button_text(
    window_query: Query<&Window>,
    effects_mode: Res<crate::visuals::VisualEffectsMode>,
    mut query: Query<(&mut Text, &mut TextFont), With<EffectsButtonText>>,
) {
    let Ok(window) = window_query.single() else { return; };
    let is_vertical = window.width() < window.height();
    if let Ok((mut text, mut font)) = query.single_mut() {
        let label = "FX";
        let new_text = match *effects_mode {
            crate::visuals::VisualEffectsMode::On => format!("{}: ON", label),
            crate::visuals::VisualEffectsMode::Off => format!("{}: OFF", label),
        };
        if text.0 != new_text {
            text.0 = new_text;
        }
        let size = if is_vertical { 14.0 } else { 18.0 };
        if font.font_size != size {
            font.font_size = size;
        }
    }
}

fn update_next_sphere_hud(
    queue: Option<Res<DispenserQueue>>,
    mut swatch_query: Query<&mut BackgroundColor, With<NextSpherePreviewSwatch>>,
    mut text_query: Query<&mut Text, With<NextSpherePreviewText>>,
) {
    let Some(queue) = queue else {
        return;
    };
    let color = TIER_COLORS[crate::visuals::tier_index(queue.next)];

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
    window_query: Query<&Window>,
    colorblind: Res<ColorblindMode>,
    mut query: Query<(&mut Text, &mut TextFont), With<ColorblindButtonText>>,
) {
    let Ok(window) = window_query.single() else { return; };
    let is_vertical = window.width() < window.height();
    if let Ok((mut text, mut font)) = query.single_mut() {
        let label = if is_vertical { "Num" } else { "Numbers" };
        let new_text = format!("{}: {}", label, if colorblind.0 { "ON" } else { "OFF" });
        if text.0 != new_text {
            text.0 = new_text;
        }
        let size = if is_vertical { 14.0 } else { 18.0 };
        if font.font_size != size {
            font.font_size = size;
        }
    }
}

fn spawn_game_over_screen(
    mut commands: Commands,
    score: Res<Score>,
    asset_server: Res<AssetServer>,
) {
    let font_handle = asset_server.load("fonts/ShareTechMono-Regular.ttf");
    commands
        .spawn((
            GameOverScreen,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(25.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            ZIndex(100),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("FATAL: board.overflow"),
                TextFont {
                    font: font_handle.clone(),
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.1, 0.1)),
            ));

            parent.spawn((
                Text::new(format!("final.score = {:06}", score.total)),
                TextFont {
                    font: font_handle.clone(),
                    font_size: 36.0,
                    ..default()
                },
                TextColor(Color::srgb(0.1, 0.9, 0.1)),
            ));

            parent.spawn((
                Text::new("[ press space to restart ]"),
                TextFont {
                    font: font_handle.clone(),
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.8, 0.5)),
            ));

            // Restart button
            parent
                .spawn((
                    RestartButton,
                    Button,
                    Node {
                        width: Val::Px(180.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        margin: UiRect::top(Val::Px(10.0)),
                        ..default()
                    },
                    BorderColor::all(Color::WHITE),
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("RESTART"),
                        TextFont {
                            font: font_handle.clone(),
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

fn despawn_game_over_screen(mut commands: Commands, query: Query<Entity, With<GameOverScreen>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn handle_restart_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<akius_core::AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        info!("Spacebar restart triggered!");
        next_state.set(akius_core::AppState::InGame);
    }
}

fn handle_restart_button(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<RestartButton>),
    >,
    mut next_state: ResMut<NextState<akius_core::AppState>>,
) {
    for (interaction, mut bg_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                info!("Restart button clicked!");
                next_state.set(akius_core::AppState::InGame);
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

fn in_game_over_or_win_state(state: Res<State<akius_core::AppState>>) -> bool {
    matches!(
        *state.get(),
        akius_core::AppState::GameOver | akius_core::AppState::Win
    )
}

fn despawn_main_menu_screen(mut commands: Commands, query: Query<Entity, With<MainMenuScreen>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn despawn_win_screen(mut commands: Commands, query: Query<Entity, With<WinScreen>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn handle_start_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<akius_core::AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        info!("Spacebar start triggered!");
        next_state.set(akius_core::AppState::InGame);
    }
}

fn handle_start_button(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<StartButton>),
    >,
    mut next_state: ResMut<NextState<akius_core::AppState>>,
) {
    for (interaction, mut bg_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                info!("Start button clicked!");
                next_state.set(akius_core::AppState::InGame);
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

fn spawn_main_menu_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font_handle = asset_server.load("fonts/ShareTechMono-Regular.ttf");
    commands
        .spawn((
            MainMenuScreen,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            ZIndex(100),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("akiuS"),
                TextFont {
                    font: font_handle.clone(),
                    font_size: 64.0,
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.9, 0.6)),
            ));

            parent.spawn((
                Text::new("[ press space to start ]"),
                TextFont {
                    font: font_handle.clone(),
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.8, 0.5)),
            ));

            // Start button
            parent
                .spawn((
                    StartButton,
                    Button,
                    Node {
                        width: Val::Px(180.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        margin: UiRect::top(Val::Px(10.0)),
                        ..default()
                    },
                    BorderColor::all(Color::WHITE),
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("START"),
                        TextFont {
                            font: font_handle.clone(),
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // Instructions Box
            parent
                .spawn((
                    Node {
                        width: Val::Percent(95.0),
                        max_width: Val::Px(680.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::FlexStart,
                        border: UiRect::all(Val::Px(2.0)),
                        padding: UiRect::all(Val::Px(15.0)),
                        margin: UiRect::top(Val::Px(15.0)),
                        row_gap: Val::Px(10.0),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.2, 0.6, 0.4)), // Muted green border
                    BackgroundColor(Color::srgba(0.02, 0.05, 0.03, 0.90)), // Dark terminal tint
                ))
                .with_children(|box_parent| {
                    box_parent.spawn((
                        Text::new("--- TELEMETRY / INSTRUCTION MANUAL ---"),
                        TextFont {
                            font: font_handle.clone(),
                            font_size: 26.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.2, 0.9, 0.6)), // Bright phosphor green
                        Node {
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        },
                    ));

                    let instructions = [
                        "[AIM]     Move cursor or press A/D (Arrows) to slide",
                        "[LAUNCH]  Click left mouse button or press SPACE to shoot",
                        "[MERGE]   Collide same-tier spheres to combine & upgrade",
                        "[FULFILL] Reach target tier [TRG] to complete orders",
                        "[HAZARD]  Avoid overflow past launcher or table spill",
                    ];

                    for inst in instructions {
                        box_parent.spawn((
                            Text::new(inst),
                            TextFont {
                                font: font_handle.clone(),
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.7, 0.9, 0.8)), // Muted terminal text
                        ));
                    }
                });
        });
}

fn spawn_win_screen(mut commands: Commands, score: Res<Score>, asset_server: Res<AssetServer>) {
    let font_handle = asset_server.load("fonts/ShareTechMono-Regular.ttf");
    commands
        .spawn((
            WinScreen,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(25.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            ZIndex(100),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("SYSTEM OK: cosmic.collapse"),
                TextFont {
                    font: font_handle.clone(),
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.75, 0.1)),
            ));

            parent.spawn((
                Text::new(format!("final.score = {:06}", score.total)),
                TextFont {
                    font: font_handle.clone(),
                    font_size: 36.0,
                    ..default()
                },
                TextColor(Color::srgb(0.1, 0.9, 0.1)),
            ));

            parent.spawn((
                Text::new("[ press space to play again ]"),
                TextFont {
                    font: font_handle.clone(),
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.8, 0.5)),
            ));

            // Restart button (labeled Restart/Play Again)
            parent
                .spawn((
                    RestartButton,
                    Button,
                    Node {
                        width: Val::Px(180.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        margin: UiRect::top(Val::Px(10.0)),
                        ..default()
                    },
                    BorderColor::all(Color::WHITE),
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("RESTART"),
                        TextFont {
                            font: font_handle.clone(),
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

fn handle_aim_guide_button(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<AimGuideButton>),
    >,
    mut aim_line_mode: ResMut<AimLineMode>,
) {
    for (interaction, mut bg_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                aim_line_mode.0 = !aim_line_mode.0;
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

fn update_aim_guide_button_text(
    window_query: Query<&Window>,
    aim_line_mode: Res<AimLineMode>,
    mut query: Query<(&mut Text, &mut TextFont), With<AimGuideButtonText>>,
) {
    let Ok(window) = window_query.single() else { return; };
    let is_vertical = window.width() < window.height();
    if let Ok((mut text, mut font)) = query.single_mut() {
        let label = if is_vertical { "Aim" } else { "Aim Line" };
        let new_text = format!("{}: {}", label, if aim_line_mode.0 { "ON" } else { "OFF" });
        if text.0 != new_text {
            text.0 = new_text;
        }
        let size = if is_vertical { 14.0 } else { 18.0 };
        if font.font_size != size {
            font.font_size = size;
        }
    }
}

fn responsive_hud_layout(
    window_query: Query<&Window>,
    mut top_row_query: Query<
        &mut Node,
        (
            With<HudTopRow>,
            Without<HudBottomRow>,
            Without<HudLeftCol>,
            Without<HudCenterCol>,
            Without<HudRightCol>,
        ),
    >,
    mut bottom_row_query: Query<
        &mut Node,
        (
            With<HudBottomRow>,
            Without<HudTopRow>,
            Without<HudLeftCol>,
            Without<HudCenterCol>,
            Without<HudRightCol>,
        ),
    >,
    mut left_col_query: Query<
        &mut Node,
        (
            With<HudLeftCol>,
            Without<HudTopRow>,
            Without<HudBottomRow>,
            Without<HudCenterCol>,
            Without<HudRightCol>,
        ),
    >,
    mut center_col_query: Query<
        &mut Node,
        (
            With<HudCenterCol>,
            Without<HudTopRow>,
            Without<HudBottomRow>,
            Without<HudLeftCol>,
            Without<HudRightCol>,
        ),
    >,
    mut right_col_query: Query<
        &mut Node,
        (
            With<HudRightCol>,
            Without<HudTopRow>,
            Without<HudBottomRow>,
            Without<HudLeftCol>,
            Without<HudCenterCol>,
        ),
    >,
    mut aim_btn_query: Query<&mut Node, (With<AimGuideButton>, Without<HudBottomRow>, Without<EffectsButton>, Without<ColorblindButton>)>,
    mut fx_btn_query: Query<&mut Node, (With<EffectsButton>, Without<HudBottomRow>, Without<AimGuideButton>, Without<ColorblindButton>)>,
    mut cb_btn_query: Query<&mut Node, (With<ColorblindButton>, Without<HudBottomRow>, Without<AimGuideButton>, Without<EffectsButton>)>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };

    let is_vertical = window.width() < window.height();

    if let Ok(mut top_row) = top_row_query.single_mut() {
        let (dir, align, justify, gap) = if is_vertical {
            (
                FlexDirection::Column,
                AlignItems::Center,
                JustifyContent::Center,
                Val::Px(8.0),
            )
        } else {
            (
                FlexDirection::Row,
                AlignItems::Center,
                JustifyContent::SpaceBetween,
                Val::Px(0.0),
            )
        };
        if top_row.flex_direction != dir {
            top_row.flex_direction = dir;
        }
        if top_row.align_items != align {
            top_row.align_items = align;
        }
        if top_row.justify_content != justify {
            top_row.justify_content = justify;
        }
        if top_row.row_gap != gap {
            top_row.row_gap = gap;
        }
    }

    if let Ok(mut bottom_row) = bottom_row_query.single_mut() {
        let (dir, align, justify, r_gap, c_gap) = if is_vertical {
            (
                FlexDirection::Row,
                AlignItems::Center,
                JustifyContent::Center,
                Val::Px(0.0),
                Val::Px(10.0),
            )
        } else {
            (
                FlexDirection::Row,
                AlignItems::Center,
                JustifyContent::FlexEnd,
                Val::Px(0.0),
                Val::Px(15.0),
            )
        };
        if bottom_row.flex_direction != dir {
            bottom_row.flex_direction = dir;
        }
        if bottom_row.align_items != align {
            bottom_row.align_items = align;
        }
        if bottom_row.justify_content != justify {
            bottom_row.justify_content = justify;
        }
        if bottom_row.row_gap != r_gap {
            bottom_row.row_gap = r_gap;
        }
        if bottom_row.column_gap != c_gap {
            bottom_row.column_gap = c_gap;
        }
    }

    let (btn_width, btn_height) = if is_vertical {
        (Val::Px(105.0), Val::Px(45.0))
    } else {
        (Val::Px(180.0), Val::Px(50.0))
    };

    if let Ok(mut node) = aim_btn_query.single_mut() {
        if node.width != btn_width { node.width = btn_width; }
        if node.height != btn_height { node.height = btn_height; }
    }
    if let Ok(mut node) = fx_btn_query.single_mut() {
        if node.width != btn_width { node.width = btn_width; }
        if node.height != btn_height { node.height = btn_height; }
    }
    if let Ok(mut node) = cb_btn_query.single_mut() {
        if node.width != btn_width { node.width = btn_width; }
        if node.height != btn_height { node.height = btn_height; }
    }

    if let Ok(mut left_col) = left_col_query.single_mut() {
        let (width, justify) = if is_vertical {
            (Val::Percent(100.0), JustifyContent::Center)
        } else {
            (Val::Percent(33.333), JustifyContent::FlexStart)
        };
        if left_col.width != width {
            left_col.width = width;
        }
        if left_col.justify_content != justify {
            left_col.justify_content = justify;
        }
    }

    if let Ok(mut center_col) = center_col_query.single_mut() {
        let width = if is_vertical {
            Val::Percent(100.0)
        } else {
            Val::Percent(33.333)
        };
        if center_col.width != width {
            center_col.width = width;
        }
    }

    if let Ok(mut right_col) = right_col_query.single_mut() {
        let (width, justify) = if is_vertical {
            (Val::Percent(100.0), JustifyContent::Center)
        } else {
            (Val::Percent(33.333), JustifyContent::FlexEnd)
        };
        if right_col.width != width {
            right_col.width = width;
        }
        if right_col.justify_content != justify {
            right_col.justify_content = justify;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akius_core::AppState;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn test_in_game_over_or_win_state() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();

        // Under MainMenu, should be false
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::MainMenu
        );
        assert!(!app
            .world_mut()
            .run_system_once(in_game_over_or_win_state)
            .unwrap());

        // Under InGame, should be false
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );
        assert!(!app
            .world_mut()
            .run_system_once(in_game_over_or_win_state)
            .unwrap());

        // Under GameOver, should be true
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::GameOver);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::GameOver
        );
        assert!(app
            .world_mut()
            .run_system_once(in_game_over_or_win_state)
            .unwrap());

        // Under Win, should be true
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Win);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Win
        );
        assert!(app
            .world_mut()
            .run_system_once(in_game_over_or_win_state)
            .unwrap());
    }
}
