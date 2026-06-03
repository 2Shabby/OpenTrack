use bevy::prelude::*;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
pub enum GameState {
    #[default]
    MainMenu,
    Setup,
    Driving,
}

#[derive(Resource, Default)]
pub struct PauseState {
    pub paused: bool,
}

pub fn not_paused(pause: Res<PauseState>) -> bool {
    !pause.paused
}
