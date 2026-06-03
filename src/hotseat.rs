use bevy::prelude::*;

use crate::driving::{CarSpawn, PlayerCar};
use crate::game_state::{GameState, not_paused};

pub struct HotseatPlugin;

impl Plugin for HotseatPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HotseatSession::default()).add_systems(
            Update,
            hotseat_controls.run_if(in_state(GameState::Driving).and(not_paused)),
        );
    }
}

#[derive(Clone, Debug)]
struct Player {
    name: String,
}

#[derive(Resource, Debug)]
pub struct HotseatSession {
    players: Vec<Player>,
    current_player: usize,
}

impl Default for HotseatSession {
    fn default() -> Self {
        Self {
            players: vec![
                Player {
                    name: "Driver 1".to_string(),
                },
                Player {
                    name: "Driver 2".to_string(),
                },
            ],
            current_player: 0,
        }
    }
}

impl HotseatSession {
    pub fn active_player_name(&self) -> &str {
        &self.players[self.current_player].name
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn configure_player_count(&mut self, player_count: usize) {
        let player_count = player_count.max(1);
        self.players = (1..=player_count)
            .map(|index| Player {
                name: format!("Driver {index}"),
            })
            .collect();
        self.current_player = 0;
    }

    fn add_player(&mut self) {
        let index = self.players.len() + 1;
        self.players.push(Player {
            name: format!("Driver {index}"),
        });
    }

    pub fn advance_player(&mut self) {
        self.current_player = (self.current_player + 1) % self.players.len();
    }
}

fn hotseat_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut hotseat: ResMut<HotseatSession>,
    car_spawn: Res<CarSpawn>,
    mut car: Single<(&mut Transform, &mut PlayerCar)>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        hotseat.add_player();
    }

    if !keys.just_pressed(KeyCode::KeyN) {
        return;
    }

    hotseat.advance_player();

    let (transform, car) = &mut *car;
    car.reset_to_spawn(transform, *car_spawn);
}
