use bevy::prelude::*;

use crate::driving::{CarSpawn, PlayerCar};
use crate::game_state::{GameState, not_paused};
use crate::run::{RunState, RunStatus};

pub struct HotseatPlugin;

impl Plugin for HotseatPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HotseatSession::default()).add_systems(
            Update,
            (record_finished_run, hotseat_controls)
                .run_if(in_state(GameState::Driving).and(not_paused)),
        );
    }
}

#[derive(Clone, Debug)]
struct Player {
    name: String,
}

#[derive(Clone, Debug)]
struct LeaderboardEntry {
    player_name: String,
    finish_time: f32,
}

#[derive(Resource, Debug)]
pub struct HotseatSession {
    players: Vec<Player>,
    current_player: usize,
    leaderboard: Vec<LeaderboardEntry>,
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
            leaderboard: Vec::new(),
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

    pub fn best_summary(&self) -> Option<(&str, f32)> {
        self.leaderboard
            .first()
            .map(|entry| (entry.player_name.as_str(), entry.finish_time))
    }

    fn add_player(&mut self) {
        let index = self.players.len() + 1;
        self.players.push(Player {
            name: format!("Driver {index}"),
        });
    }

    fn advance_player(&mut self) {
        self.current_player = (self.current_player + 1) % self.players.len();
    }
}

fn record_finished_run(mut run: ResMut<RunState>, mut hotseat: ResMut<HotseatSession>) {
    if run.status != RunStatus::Finished || run.finish_recorded {
        return;
    }

    let entry = LeaderboardEntry {
        player_name: hotseat.active_player_name().to_string(),
        finish_time: run.elapsed,
    };

    hotseat.leaderboard.push(entry);
    hotseat
        .leaderboard
        .sort_by(|a, b| a.finish_time.total_cmp(&b.finish_time));
    run.finish_recorded = true;
}

fn hotseat_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<RunState>,
    mut hotseat: ResMut<HotseatSession>,
    car_spawn: Res<CarSpawn>,
    mut car: Single<(&mut Transform, &mut PlayerCar)>,
) {
    if keys.just_pressed(KeyCode::KeyP) && run.status == RunStatus::Waiting {
        hotseat.add_player();
    }

    if !keys.just_pressed(KeyCode::KeyN) || run.status != RunStatus::Finished {
        return;
    }

    hotseat.advance_player();
    run.reset();

    let (transform, car) = &mut *car;
    car.reset_to_spawn(transform, *car_spawn);
}
