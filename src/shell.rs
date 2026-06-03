mod main_menu;
mod pause;
mod results;
mod setup;
mod ui;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};

use crate::game_state::{GameState, PauseState};
pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseState>()
            .init_resource::<QuitRequested>()
            .init_resource::<SessionSetup>()
            .add_systems(OnEnter(GameState::MainMenu), main_menu::spawn)
            .add_systems(OnEnter(GameState::Setup), setup::spawn)
            .add_systems(OnEnter(GameState::Results), results::spawn)
            .add_systems(OnEnter(GameState::Driving), pause::clear)
            .add_systems(
                OnExit(GameState::Driving),
                (pause::clear, despawn_screen::<pause::PauseMenuEntity>),
            )
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
            .add_systems(Update, request_primary_window_close)
            .add_systems(
                OnExit(GameState::MainMenu),
                despawn_screen::<main_menu::MainMenuEntity>,
            )
            .add_systems(
                OnExit(GameState::Setup),
                despawn_screen::<setup::SetupEntity>,
            )
            .add_systems(
                OnExit(GameState::Results),
                despawn_screen::<results::ResultsEntity>,
            );
    }
}

#[derive(Resource, Default)]
pub(super) struct QuitRequested {
    requested: bool,
}

impl QuitRequested {
    pub fn request(&mut self) {
        self.requested = true;
    }
}

fn request_primary_window_close(
    mut quit: ResMut<QuitRequested>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    close_requested: Option<MessageWriter<WindowCloseRequested>>,
) {
    if !quit.requested {
        return;
    }

    let Some(mut close_requested) = close_requested else {
        return;
    };

    if let Ok(window) = primary_window.single() {
        close_requested.write(WindowCloseRequested { window });
        quit.requested = false;
    }
}

fn despawn_screen<T: Component>(mut commands: Commands, entities: Query<Entity, With<T>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

#[derive(Resource)]
pub(super) struct SessionSetup {
    pub player_count: usize,
    pub seed: u64,
    pub piece_count: usize,
    pub vehicle_index: usize,
}

impl Default for SessionSetup {
    fn default() -> Self {
        Self {
            player_count: 2,
            seed: 0x5EED_2026,
            piece_count: 8,
            vehicle_index: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    use crate::car_asset::VehicleSelection;
    use crate::driving::CarSpawn;
    use crate::hotseat::HotseatSession;
    use crate::run::RunState;
    use crate::track::TrackRecipe;

    #[test]
    fn leaving_driving_despawns_pause_menu() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.insert_resource(TrackRecipe::default());
        app.insert_resource(HotseatSession::default());
        app.insert_resource(VehicleSelection::default());
        app.insert_resource(RunState::new(1));
        app.insert_resource(CarSpawn::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(ShellPlugin);

        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Driving);
        app.update();

        app.world_mut().resource_mut::<PauseState>().paused = true;
        app.update();
        assert!(pause_menu_count(&mut app) > 0);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Setup);
        app.update();
        assert_eq!(pause_menu_count(&mut app), 0);
    }

    fn pause_menu_count(app: &mut App) -> usize {
        let mut menus = app
            .world_mut()
            .query_filtered::<(), With<pause::PauseMenuEntity>>();
        menus.iter(app.world()).count()
    }
}
