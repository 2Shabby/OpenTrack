use bevy::prelude::*;

use crate::driving::PlayerCar;
use crate::ghost::SessionBestGhost;
use crate::hotseat::HotseatSession;
use crate::run::RunState;
use crate::surface::SurfaceLibrary;
use crate::track::GeneratedTrackInfo;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_debug_overlay)
            .add_systems(Update, update_debug_overlay);
    }
}

#[derive(Component)]
struct DebugOverlay;

fn spawn_debug_overlay(mut commands: Commands) {
    commands.spawn((
        Text::new("debug"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
        DebugOverlay,
    ));
}

fn update_debug_overlay(
    car: Single<&PlayerCar>,
    run: Res<RunState>,
    ghost: Res<SessionBestGhost>,
    hotseat: Res<HotseatSession>,
    surfaces: Res<SurfaceLibrary>,
    track: Res<GeneratedTrackInfo>,
    mut overlay: Single<&mut Text, With<DebugOverlay>>,
) {
    let params = surfaces.get(car.current_surface);
    let best = hotseat
        .best_summary()
        .map(|(name, finish_time)| format!("{name} {finish_time:.2}"))
        .unwrap_or_else(|| "none".to_string());
    let ghost_best = ghost
        .finish_time()
        .map(|time| format!("{time:.2}"))
        .unwrap_or_else(|| "none".to_string());

    overlay.0 = format!(
        "seed: {}\npieces: {}\ntrack cps: {}\nplayer: {}\nplayers: {}\nbest: {}\nghost: {}\ntime: {:>6.2}\nrun: {}\ncheckpoint: {}/{}\nspeed: {:>5.1}\nsigned: {:+5.1}\nmode: {}\nsurface: {}\nthrottle: {:+.0}\nsteer: {:+.0}\nlat grip: {:.2}\naccel mult: {:.2}",
        track.seed,
        track.piece_count,
        track.checkpoint_count,
        hotseat.active_player_name(),
        hotseat.player_count(),
        best,
        ghost_best,
        run.elapsed,
        run.status_label(),
        run.next_checkpoint,
        run.checkpoint_count,
        car.velocity.length(),
        car.signed_speed,
        car.drive_mode.label(),
        car.current_surface.label(),
        car.throttle,
        car.steer,
        params.lateral_grip,
        params.acceleration_multiplier,
    );
}
