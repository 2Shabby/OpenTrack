use bevy::app::AppExit;
use bevy::prelude::*;

use crate::game_state::GameState;

pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(
                Update,
                handle_main_menu.run_if(in_state(GameState::MainMenu)),
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
