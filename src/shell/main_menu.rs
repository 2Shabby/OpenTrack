use bevy::app::AppExit;
use bevy::prelude::*;

use crate::game_state::GameState;

type MenuButtons<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static MainMenuAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
pub(super) struct MainMenuEntity;

#[derive(Clone, Copy, Component)]
pub(super) enum MainMenuAction {
    StartHotseat,
    Quit,
}

pub(super) fn spawn(mut commands: Commands) {
    commands.spawn((Camera2d, MainMenuEntity));

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

            button(parent, "Start Hotseat", MainMenuAction::StartHotseat);
            button(parent, "Quit", MainMenuAction::Quit);
        });
}

pub(super) fn handle(
    mut next_state: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    buttons: MenuButtons,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            MainMenuAction::StartHotseat => next_state.set(GameState::Setup),
            MainMenuAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

pub(super) fn despawn(mut commands: Commands, entities: Query<Entity, With<MainMenuEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn button(parent: &mut ChildSpawnerCommands, label: &str, action: MainMenuAction) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    use crate::hotseat::HotseatPlugin;
    use crate::run::RunPlugin;
    use crate::shell::ShellPlugin;

    #[test]
    fn main_menu_spawns_ui_camera() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.add_plugins((RunPlugin, HotseatPlugin, ShellPlugin));

        app.update();

        let mut cameras = app
            .world_mut()
            .query_filtered::<(), (With<Camera2d>, With<MainMenuEntity>)>();
        assert_eq!(cameras.iter(app.world()).count(), 1);
    }
}
