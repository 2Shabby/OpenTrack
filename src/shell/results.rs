use bevy::prelude::*;

use super::QuitRequested;
use super::ui::{self, ButtonSpec};
use crate::game_state::GameState;
use crate::hotseat::HotseatSession;
use crate::run::{RunState, RunStatus};

type ResultsButtons<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static ResultsAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
pub(super) struct ResultsEntity;

#[derive(Clone, Copy, Component)]
pub(super) enum ResultsAction {
    Retry,
    NextPlayer,
    MainMenu,
    Quit,
}

pub(super) fn spawn(mut commands: Commands, run: Res<RunState>, hotseat: Res<HotseatSession>) {
    commands.spawn((Camera2d, ResultsEntity));

    let leaderboard = hotseat.leaderboard_lines();
    let leaderboard_text = if leaderboard.is_empty() {
        "No finishes yet".to_string()
    } else {
        leaderboard.join("\n")
    };

    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(14),
                ..default()
            },
            BackgroundColor(Color::srgb(0.035, 0.04, 0.045)),
            ResultsEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Results"),
                TextFont {
                    font_size: 42.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new(format!(
                    "{} finished in {:.2}",
                    hotseat.active_player_name(),
                    run.elapsed
                )),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new(leaderboard_text),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.82, 0.86, 0.86)),
            ));

            button(parent, "Retry", ResultsAction::Retry);
            button(parent, "Next Player", ResultsAction::NextPlayer);
            button(parent, "Main Menu", ResultsAction::MainMenu);
            button(parent, "Quit", ResultsAction::Quit);
        });
}

pub(super) fn enter_after_finish(run: Res<RunState>, mut next_state: ResMut<NextState<GameState>>) {
    if run.status == RunStatus::Finished && run.finish_recorded {
        next_state.set(GameState::Results);
    }
}

pub(super) fn handle(
    mut run: ResMut<RunState>,
    mut hotseat: ResMut<HotseatSession>,
    mut next_state: ResMut<NextState<GameState>>,
    mut quit: ResMut<QuitRequested>,
    buttons: ResultsButtons,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            ResultsAction::Retry => {
                run.reset();
                next_state.set(GameState::Driving);
            }
            ResultsAction::NextPlayer => {
                hotseat.advance_player();
                run.reset();
                next_state.set(GameState::Driving);
            }
            ResultsAction::MainMenu => {
                run.reset();
                next_state.set(GameState::MainMenu);
            }
            ResultsAction::Quit => {
                quit.request();
            }
        }
    }
}

fn button(parent: &mut ChildSpawnerCommands, label: &str, action: ResultsAction) {
    ui::button(
        parent,
        label,
        action,
        ButtonSpec {
            node: Node {
                width: px(220),
                height: px(42),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            font_size: 18.0,
            background: Color::srgb(0.15, 0.18, 0.19),
        },
    );
}
