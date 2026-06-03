use bevy::app::AppExit;
use bevy::prelude::*;

use crate::driving::{CarSpawn, PlayerCar};
use crate::game_state::{GameState, PauseState};
use crate::run::RunState;

type PauseButtons<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static PauseMenuAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
pub(super) struct PauseMenuEntity;

#[derive(Clone, Copy, Component)]
pub(super) enum PauseMenuAction {
    Resume,
    Restart,
    Setup,
    MainMenu,
    Quit,
}

pub(super) fn clear(mut pause: ResMut<PauseState>) {
    pause.paused = false;
}

pub(super) fn toggle_from_keyboard(keys: Res<ButtonInput<KeyCode>>, mut pause: ResMut<PauseState>) {
    if keys.just_pressed(KeyCode::Escape) {
        pause.paused = !pause.paused;
    }
}

pub(super) fn sync_menu(
    mut commands: Commands,
    pause: Res<PauseState>,
    menu: Query<Entity, (With<PauseMenuEntity>, Without<ChildOf>)>,
) {
    if !pause.is_changed() {
        return;
    }

    let has_menu = !menu.is_empty();
    if pause.paused && !has_menu {
        spawn_menu(&mut commands);
    } else if !pause.paused && has_menu {
        for entity in &menu {
            commands.entity(entity).despawn();
        }
    }
}

pub(super) fn handle_menu(
    mut pause: ResMut<PauseState>,
    mut run: ResMut<RunState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    car_spawn: Res<CarSpawn>,
    mut cars: Query<(&mut Transform, &mut PlayerCar)>,
    buttons: PauseButtons,
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
            PauseMenuAction::Restart => {
                run.reset();
                reset_player_car(&mut cars, *car_spawn);
                pause.paused = false;
            }
            PauseMenuAction::Setup => {
                run.reset();
                pause.paused = false;
                next_state.set(GameState::Setup);
            }
            PauseMenuAction::MainMenu => {
                run.reset();
                pause.paused = false;
                next_state.set(GameState::MainMenu);
            }
            PauseMenuAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

pub(super) fn despawn(
    mut commands: Commands,
    entities: Query<Entity, (With<PauseMenuEntity>, Without<ChildOf>)>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn reset_player_car(cars: &mut Query<(&mut Transform, &mut PlayerCar)>, car_spawn: CarSpawn) {
    let Some((mut transform, mut car)) = cars.iter_mut().next() else {
        return;
    };

    car.reset_to_spawn(&mut transform, car_spawn);
}

fn spawn_menu(commands: &mut Commands) {
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

            button(parent, "Resume", PauseMenuAction::Resume);
            button(parent, "Restart", PauseMenuAction::Restart);
            button(parent, "Setup", PauseMenuAction::Setup);
            button(parent, "Main Menu", PauseMenuAction::MainMenu);
            button(parent, "Quit", PauseMenuAction::Quit);
        });
}

fn button(parent: &mut ChildSpawnerCommands, label: &str, action: PauseMenuAction) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_menu_despawn_removes_menu_entities() {
        let mut app = App::new();
        app.add_systems(Update, despawn);
        app.world_mut().spawn(PauseMenuEntity);

        app.update();

        let mut menus = app
            .world_mut()
            .query_filtered::<(), With<PauseMenuEntity>>();
        assert_eq!(menus.iter(app.world()).count(), 0);
    }
}
