mod main_menu;
mod pause;
mod results;
mod setup;

use bevy::prelude::*;

use crate::game_state::{GameState, PauseState};
pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseState>()
            .init_resource::<SessionSetup>()
            .add_systems(OnEnter(GameState::MainMenu), main_menu::spawn)
            .add_systems(OnEnter(GameState::Setup), setup::spawn)
            .add_systems(OnEnter(GameState::Results), results::spawn)
            .add_systems(OnEnter(GameState::Driving), pause::clear)
            .add_systems(
                Update,
                main_menu::handle.run_if(in_state(GameState::MainMenu)),
            )
            .add_systems(
                Update,
                (setup::handle, setup::update_values).run_if(in_state(GameState::Setup)),
            )
            .add_systems(
                Update,
                (
                    results::enter_after_finish,
                    pause::toggle_from_keyboard,
                    pause::sync_menu,
                    pause::handle_menu,
                )
                    .run_if(in_state(GameState::Driving)),
            )
            .add_systems(Update, results::handle.run_if(in_state(GameState::Results)))
            .add_systems(OnExit(GameState::MainMenu), main_menu::despawn)
            .add_systems(OnExit(GameState::Setup), setup::despawn)
            .add_systems(OnExit(GameState::Results), results::despawn);
    }
}

#[derive(Resource)]
pub(super) struct SessionSetup {
    pub player_count: usize,
    pub seed: u64,
    pub piece_count: usize,
    pub car_color_index: usize,
}

impl Default for SessionSetup {
    fn default() -> Self {
        Self {
            player_count: 2,
            seed: 0x5EED_2026,
            piece_count: 8,
            car_color_index: 0,
        }
    }
}
