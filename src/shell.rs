use bevy::app::AppExit;
use bevy::prelude::*;

use crate::driving::CarPaint;
use crate::game_state::{GameState, PauseState};
use crate::hotseat::HotseatSession;
use crate::run::{RunState, RunStatus};
use crate::track::{SurfaceMix, TrackRecipe};

pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseState>()
            .init_resource::<SessionSetup>()
            .add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(OnEnter(GameState::Setup), spawn_setup_screen)
            .add_systems(OnEnter(GameState::Results), spawn_results_screen)
            .add_systems(OnEnter(GameState::Driving), clear_pause)
            .add_systems(
                Update,
                handle_main_menu.run_if(in_state(GameState::MainMenu)),
            )
            .add_systems(
                Update,
                (handle_setup_screen, update_setup_values).run_if(in_state(GameState::Setup)),
            )
            .add_systems(
                Update,
                (
                    enter_results_after_finish,
                    toggle_pause_from_keyboard,
                    sync_pause_menu,
                    handle_pause_menu,
                )
                    .run_if(in_state(GameState::Driving)),
            )
            .add_systems(
                Update,
                handle_results_screen.run_if(in_state(GameState::Results)),
            )
            .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
            .add_systems(OnExit(GameState::Setup), despawn_setup_screen)
            .add_systems(OnExit(GameState::Results), despawn_results_screen);
    }
}

#[derive(Resource)]
struct SessionSetup {
    player_count: usize,
    seed: u64,
    piece_count: usize,
    difficulty: u8,
    surface_mix: SurfaceMix,
    car_color_index: usize,
}

impl Default for SessionSetup {
    fn default() -> Self {
        Self {
            player_count: 2,
            seed: 0x5EED_2026,
            piece_count: 8,
            difficulty: 1,
            surface_mix: SurfaceMix::Balanced,
            car_color_index: 0,
        }
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
struct SetupEntity;

#[derive(Clone, Copy, Component)]
enum SetupAction {
    PlayersDown,
    PlayersUp,
    SeedDown,
    SeedUp,
    LengthDown,
    LengthUp,
    DifficultyDown,
    DifficultyUp,
    SurfaceMixDown,
    SurfaceMixUp,
    ColorDown,
    ColorUp,
    StartRace,
    Back,
}

#[derive(Clone, Copy, Component)]
enum SetupValue {
    Players,
    Seed,
    Length,
    Difficulty,
    SurfaceMix,
    Color,
}

#[derive(Component)]
struct ResultsEntity;

#[derive(Clone, Copy, Component)]
enum ResultsAction {
    Retry,
    NextPlayer,
    MainMenu,
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

fn spawn_setup_screen(mut commands: Commands) {
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

            setup_row(
                parent,
                "Players",
                SetupValue::Players,
                SetupAction::PlayersDown,
                SetupAction::PlayersUp,
            );
            setup_row(
                parent,
                "Seed",
                SetupValue::Seed,
                SetupAction::SeedDown,
                SetupAction::SeedUp,
            );
            setup_row(
                parent,
                "Length",
                SetupValue::Length,
                SetupAction::LengthDown,
                SetupAction::LengthUp,
            );
            setup_row(
                parent,
                "Difficulty",
                SetupValue::Difficulty,
                SetupAction::DifficultyDown,
                SetupAction::DifficultyUp,
            );
            setup_row(
                parent,
                "Surface",
                SetupValue::SurfaceMix,
                SetupAction::SurfaceMixDown,
                SetupAction::SurfaceMixUp,
            );
            setup_row(
                parent,
                "Color",
                SetupValue::Color,
                SetupAction::ColorDown,
                SetupAction::ColorUp,
            );

            setup_button(parent, "Start Race", SetupAction::StartRace);
            setup_button(parent, "Back", SetupAction::Back);
        });
}

fn spawn_results_screen(mut commands: Commands, run: Res<RunState>, hotseat: Res<HotseatSession>) {
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
                ResultsEntity,
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
                ResultsEntity,
            ));
            parent.spawn((
                Text::new(leaderboard_text),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.82, 0.86, 0.86)),
                ResultsEntity,
            ));

            results_button(parent, "Retry", ResultsAction::Retry);
            results_button(parent, "Next Player", ResultsAction::NextPlayer);
            results_button(parent, "Main Menu", ResultsAction::MainMenu);
            results_button(parent, "Quit", ResultsAction::Quit);
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

fn setup_row(
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
            setup_button(parent, "-", down);
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
            setup_button(parent, "+", up);
        });
}

fn setup_button(parent: &mut ChildSpawnerCommands, label: &str, action: SetupAction) {
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

fn results_button(parent: &mut ChildSpawnerCommands, label: &str, action: ResultsAction) {
    parent
        .spawn((
            Button,
            Node {
                width: px(220),
                height: px(42),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.18, 0.19)),
            action,
            ResultsEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(label),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ResultsEntity,
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
            MainMenuAction::StartHotseat => next_state.set(GameState::Setup),
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

fn handle_setup_screen(
    mut setup: ResMut<SessionSetup>,
    mut recipe: ResMut<TrackRecipe>,
    mut hotseat: ResMut<HotseatSession>,
    mut car_paint: ResMut<CarPaint>,
    mut run: ResMut<RunState>,
    mut next_state: ResMut<NextState<GameState>>,
    buttons: Query<(&Interaction, &SetupAction), (Changed<Interaction>, With<Button>)>,
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
            SetupAction::DifficultyDown => {
                setup.difficulty = setup.difficulty.saturating_sub(1);
            }
            SetupAction::DifficultyUp => setup.difficulty = (setup.difficulty + 1).min(3),
            SetupAction::SurfaceMixDown => setup.surface_mix = setup.surface_mix.previous(),
            SetupAction::SurfaceMixUp => setup.surface_mix = setup.surface_mix.next(),
            SetupAction::ColorDown => {
                setup.car_color_index = setup.car_color_index.saturating_sub(1);
            }
            SetupAction::ColorUp => {
                setup.car_color_index = (setup.car_color_index + 1).min(car_color_count() - 1);
            }
            SetupAction::StartRace => {
                recipe.seed = setup.seed;
                recipe.piece_count = setup.piece_count;
                recipe.difficulty = setup.difficulty;
                recipe.surface_mix = setup.surface_mix;
                hotseat.configure_player_count(setup.player_count);
                car_paint.color = car_color(setup.car_color_index);
                run.reset();
                next_state.set(GameState::Driving);
            }
            SetupAction::Back => next_state.set(GameState::MainMenu),
        }
    }
}

fn update_setup_values(setup: Res<SessionSetup>, mut values: Query<(&mut Text, &SetupValue)>) {
    for (mut text, value) in &mut values {
        text.0 = match value {
            SetupValue::Players => setup.player_count.to_string(),
            SetupValue::Seed => setup.seed.to_string(),
            SetupValue::Length => setup.piece_count.to_string(),
            SetupValue::Difficulty => setup.difficulty.to_string(),
            SetupValue::SurfaceMix => setup.surface_mix.label().to_string(),
            SetupValue::Color => car_color_name(setup.car_color_index).to_string(),
        };
    }
}

fn despawn_setup_screen(mut commands: Commands, entities: Query<Entity, With<SetupEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn enter_results_after_finish(run: Res<RunState>, mut next_state: ResMut<NextState<GameState>>) {
    if run.status == RunStatus::Finished && run.finish_recorded {
        next_state.set(GameState::Results);
    }
}

fn handle_results_screen(
    mut run: ResMut<RunState>,
    mut hotseat: ResMut<HotseatSession>,
    mut next_state: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    buttons: Query<(&Interaction, &ResultsAction), (Changed<Interaction>, With<Button>)>,
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
                exit.write(AppExit::Success);
            }
        }
    }
}

fn despawn_results_screen(mut commands: Commands, entities: Query<Entity, With<ResultsEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn car_color_count() -> usize {
    5
}

fn car_color_name(index: usize) -> &'static str {
    match index {
        0 => "Red",
        1 => "Blue",
        2 => "Green",
        3 => "Yellow",
        _ => "White",
    }
}

fn car_color(index: usize) -> Color {
    match index {
        0 => Color::srgb(0.92, 0.08, 0.05),
        1 => Color::srgb(0.08, 0.24, 0.92),
        2 => Color::srgb(0.08, 0.62, 0.2),
        3 => Color::srgb(0.95, 0.78, 0.08),
        _ => Color::srgb(0.9, 0.9, 0.86),
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
