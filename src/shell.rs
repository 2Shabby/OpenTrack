use bevy::app::AppExit;
use bevy::prelude::*;

use crate::game_state::{GameState, PauseState};

pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseState>()
            .add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(OnEnter(GameState::Driving), clear_pause)
            .add_systems(
                Update,
                handle_main_menu.run_if(in_state(GameState::MainMenu)),
            )
            .add_systems(
                Update,
                (
                    toggle_pause_from_keyboard,
                    sync_pause_menu,
                    handle_pause_menu,
                )
                    .run_if(in_state(GameState::Driving)),
            )
            .add_systems(OnExit(GameState::MainMenu), despawn_main_menu);
    }
}

#[derive(Component)]
struct MainMenuEntity;

#[derive(Clone, Copy, Component)]
enum MainMenuAction {
    StartHotseat,
    Quit,
}

#[derive(Component)]
struct PauseMenuEntity;

#[derive(Clone, Copy, Component)]
enum PauseMenuAction {
    Resume,
    MainMenu,
    Quit,
}

fn spawn_main_menu(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                ..default()
            },
            BackgroundColor(Color::srgb(0.035, 0.04, 0.045)),
            MainMenuEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Open Track Turbo"),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                MainMenuEntity,
            ));

            menu_button(parent, "Start Hotseat", MainMenuAction::StartHotseat);
            menu_button(parent, "Quit", MainMenuAction::Quit);
        });
}

fn menu_button(parent: &mut ChildSpawnerCommands, label: &str, action: MainMenuAction) {
    parent
        .spawn((
            Button,
            Node {
                width: px(240),
                height: px(48),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.18, 0.19)),
            action,
            MainMenuEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(label),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                MainMenuEntity,
            ));
        });
}

fn pause_button(parent: &mut ChildSpawnerCommands, label: &str, action: PauseMenuAction) {
    parent
        .spawn((
            Button,
            Node {
                width: px(220),
                height: px(44),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.16, 0.18, 0.19)),
            action,
            PauseMenuEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(label),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                PauseMenuEntity,
            ));
        });
}

fn handle_main_menu(
    mut next_state: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    buttons: Query<(&Interaction, &MainMenuAction), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            MainMenuAction::StartHotseat => next_state.set(GameState::Driving),
            MainMenuAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

fn despawn_main_menu(mut commands: Commands, entities: Query<Entity, With<MainMenuEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn clear_pause(mut pause: ResMut<PauseState>) {
    pause.paused = false;
}

fn toggle_pause_from_keyboard(keys: Res<ButtonInput<KeyCode>>, mut pause: ResMut<PauseState>) {
    if keys.just_pressed(KeyCode::Escape) {
        pause.paused = !pause.paused;
    }
}

fn sync_pause_menu(
    mut commands: Commands,
    pause: Res<PauseState>,
    menu: Query<Entity, With<PauseMenuEntity>>,
) {
    if !pause.is_changed() {
        return;
    }

    let has_menu = !menu.is_empty();
    if pause.paused && !has_menu {
        spawn_pause_menu(&mut commands);
    } else if !pause.paused && has_menu {
        for entity in &menu {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_pause_menu(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(14),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.025, 0.03, 0.76)),
            PauseMenuEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Paused"),
                TextFont {
                    font_size: 38.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                PauseMenuEntity,
            ));

            pause_button(parent, "Resume", PauseMenuAction::Resume);
            pause_button(parent, "Main Menu", PauseMenuAction::MainMenu);
            pause_button(parent, "Quit", PauseMenuAction::Quit);
        });
}

fn handle_pause_menu(
    mut pause: ResMut<PauseState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    buttons: Query<(&Interaction, &PauseMenuAction), (Changed<Interaction>, With<Button>)>,
) {
    if !pause.paused {
        return;
    }

    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            PauseMenuAction::Resume => pause.paused = false,
            PauseMenuAction::MainMenu => {
                pause.paused = false;
                next_state.set(GameState::MainMenu);
            }
            PauseMenuAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}
