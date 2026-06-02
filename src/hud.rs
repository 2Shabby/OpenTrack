use bevy::prelude::*;

use crate::driving::PlayerCar;
use crate::game_state::{GameState, not_paused};
use crate::ghost::SessionBestGhost;
use crate::hotseat::HotseatSession;
use crate::run::RunState;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Driving), spawn_hud)
            .add_systems(
                Update,
                update_hud.run_if(in_state(GameState::Driving).and(not_paused)),
            )
            .add_systems(OnExit(GameState::Driving), despawn_hud);
    }
}

#[derive(Component)]
struct HudEntity;

#[derive(Component)]
struct HudText;

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("hud"),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            right: px(12),
            ..default()
        },
        HudText,
        HudEntity,
    ));
}

fn update_hud(
    car: Single<&PlayerCar>,
    run: Res<RunState>,
    hotseat: Res<HotseatSession>,
    ghost: Res<SessionBestGhost>,
    mut hud: Single<&mut Text, With<HudText>>,
) {
    let best = hotseat
        .best_summary()
        .map(|(name, finish_time)| format!("{name} {finish_time:.2}"))
        .unwrap_or_else(|| "none".to_string());
    let ghost = ghost
        .finish_time()
        .map(|finish_time| format!("{finish_time:.2}"))
        .unwrap_or_else(|| "none".to_string());

    hud.0 = format!(
        "{}\nTime {:>6.2}\nCheckpoint {}/{}\nSpeed {:>5.1}\nBest {}\nGhost {}",
        hotseat.active_player_name(),
        run.elapsed,
        run.next_checkpoint,
        run.checkpoint_count,
        car.velocity.length(),
        best,
        ghost
    );
}

fn despawn_hud(mut commands: Commands, entities: Query<Entity, With<HudEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}
