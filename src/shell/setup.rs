use bevy::prelude::*;

use super::SessionSetup;
use crate::car_asset::{VehicleKind, VehicleSelection};
use crate::game_state::GameState;
use crate::hotseat::HotseatSession;
use crate::run::RunState;
use crate::track::TrackRecipe;

type SetupButtons<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static SetupAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
pub(super) struct SetupEntity;

#[derive(Clone, Copy, Component)]
pub(super) enum SetupAction {
    PlayersDown,
    PlayersUp,
    SeedDown,
    SeedUp,
    LengthDown,
    LengthUp,
    VehicleDown,
    VehicleUp,
    StartRace,
    Back,
}

#[derive(Clone, Copy, Component)]
pub(super) enum SetupValue {
    Players,
    Seed,
    Length,
    Vehicle,
}

pub(super) fn spawn(mut commands: Commands) {
    commands.spawn((Camera2d, SetupEntity));

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
            SetupEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Session Setup"),
                TextFont {
                    font_size: 42.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                SetupEntity,
            ));

            row(
                parent,
                "Players",
                SetupValue::Players,
                SetupAction::PlayersDown,
                SetupAction::PlayersUp,
            );
            row(
                parent,
                "Seed",
                SetupValue::Seed,
                SetupAction::SeedDown,
                SetupAction::SeedUp,
            );
            row(
                parent,
                "Length",
                SetupValue::Length,
                SetupAction::LengthDown,
                SetupAction::LengthUp,
            );
            row(
                parent,
                "Vehicle",
                SetupValue::Vehicle,
                SetupAction::VehicleDown,
                SetupAction::VehicleUp,
            );

            button(parent, "Start Race", SetupAction::StartRace);
            button(parent, "Back", SetupAction::Back);
        });
}

pub(super) fn handle(
    mut setup: ResMut<SessionSetup>,
    mut recipe: ResMut<TrackRecipe>,
    mut hotseat: ResMut<HotseatSession>,
    mut vehicle_selection: ResMut<VehicleSelection>,
    mut run: ResMut<RunState>,
    mut next_state: ResMut<NextState<GameState>>,
    buttons: SetupButtons,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            SetupAction::PlayersDown => {
                setup.player_count = setup.player_count.saturating_sub(1).max(1)
            }
            SetupAction::PlayersUp => setup.player_count = (setup.player_count + 1).min(16),
            SetupAction::SeedDown => setup.seed = setup.seed.saturating_sub(1),
            SetupAction::SeedUp => setup.seed = setup.seed.saturating_add(1),
            SetupAction::LengthDown => {
                setup.piece_count = setup.piece_count.saturating_sub(1).max(4)
            }
            SetupAction::LengthUp => setup.piece_count = (setup.piece_count + 1).min(64),
            SetupAction::VehicleDown => {
                setup.vehicle_index = setup.vehicle_index.saturating_sub(1);
            }
            SetupAction::VehicleUp => {
                setup.vehicle_index = (setup.vehicle_index + 1).min(VehicleKind::count() - 1);
            }
            SetupAction::StartRace => {
                recipe.seed = setup.seed;
                recipe.piece_count = setup.piece_count;
                hotseat.configure_player_count(setup.player_count);
                vehicle_selection.vehicle = VehicleKind::from_index(setup.vehicle_index);
                run.reset();
                next_state.set(GameState::Driving);
            }
            SetupAction::Back => next_state.set(GameState::MainMenu),
        }
    }
}

pub(super) fn update_values(setup: Res<SessionSetup>, mut values: Query<(&mut Text, &SetupValue)>) {
    for (mut text, value) in &mut values {
        text.0 = match value {
            SetupValue::Players => setup.player_count.to_string(),
            SetupValue::Seed => setup.seed.to_string(),
            SetupValue::Length => setup.piece_count.to_string(),
            SetupValue::Vehicle => VehicleKind::from_index(setup.vehicle_index)
                .name()
                .to_string(),
        };
    }
}

pub(super) fn despawn(
    mut commands: Commands,
    entities: Query<Entity, (With<SetupEntity>, Without<ChildOf>)>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: SetupValue,
    down: SetupAction,
    up: SetupAction,
) {
    parent
        .spawn((
            Node {
                width: px(420),
                height: px(44),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(12),
                ..default()
            },
            SetupEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(label),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                SetupEntity,
            ));
            button(parent, "-", down);
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                value,
                SetupEntity,
            ));
            button(parent, "+", up);
        });
}

fn button(parent: &mut ChildSpawnerCommands, label: &str, action: SetupAction) {
    parent
        .spawn((
            Button,
            Node {
                min_width: px(44),
                height: px(40),
                padding: UiRect::axes(px(12), px(0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.18, 0.19)),
            action,
            SetupEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(label),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                SetupEntity,
            ));
        });
}
