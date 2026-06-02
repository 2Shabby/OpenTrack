use bevy::prelude::*;

use crate::driving::PlayerCar;
use crate::game_state::GameState;
use crate::ghost::SessionBestGhost;
use crate::hotseat::HotseatSession;
use crate::run::RunState;
use crate::surface::SurfaceLibrary;
use crate::track::{GeneratedRail, GeneratedRoadSurface, GeneratedTrackInfo, GeneratedTrigger};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugOverlayState>()
            .add_systems(OnEnter(GameState::Driving), spawn_debug_overlay)
            .add_systems(
                Update,
                (toggle_debug_overlay, update_debug_overlay).run_if(in_state(GameState::Driving)),
            )
            .add_systems(OnExit(GameState::Driving), despawn_debug_overlay);
    }
}

#[derive(Resource, Default)]
struct DebugOverlayState {
    visible: bool,
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
        Visibility::Hidden,
        DebugOverlay,
    ));
}

fn despawn_debug_overlay(mut commands: Commands, overlays: Query<Entity, With<DebugOverlay>>) {
    for entity in &overlays {
        commands.entity(entity).despawn();
    }
}

fn update_debug_overlay(
    debug: Res<DebugOverlayState>,
    car: Single<&PlayerCar>,
    run: Res<RunState>,
    ghost: Res<SessionBestGhost>,
    hotseat: Res<HotseatSession>,
    surfaces: Res<SurfaceLibrary>,
    track: Res<GeneratedTrackInfo>,
    road_surfaces: Query<(), With<GeneratedRoadSurface>>,
    rails: Query<(), With<GeneratedRail>>,
    triggers: Query<(), With<GeneratedTrigger>>,
    mut overlay: Single<(&mut Text, &mut Visibility), With<DebugOverlay>>,
) {
    overlay.1.set_if_neq(if debug.visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    });
    if !debug.visible {
        return;
    }

    let params = surfaces.get(car.current_surface);
    let best = hotseat
        .best_summary()
        .map(|(name, finish_time)| format!("{name} {finish_time:.2}"))
        .unwrap_or_else(|| "none".to_string());
    let ghost_best = ghost
        .finish_time()
        .map(|time| format!("{time:.2}"))
        .unwrap_or_else(|| "none".to_string());

    overlay.0.0 = format!(
        "seed: {}\npieces: {}\ntrack cps: {}\nroad: {}/{}\nrail: {}/{}\ntrigger: {}/{}\nplayer: {}\nplayers: {}\nbest: {}\nghost: {}\ntime: {:>6.2}\nrun: {}\ncheckpoint: {}/{}\nspeed: {:>5.1}\nsigned: {:+5.1}\nmode: {}\nhandling: {}\nslip: {:>4.0} deg\nground: {} {}\nwheels: {}\nsplit: {}\nthrottle: {:+.0}\nsteer: {:+.0}\nlat grip: {:.2}\naccel mult: {:.2}",
        track.seed,
        track.piece_count,
        track.checkpoint_count,
        road_surfaces.iter().count(),
        track.road_surface_count,
        rails.iter().count(),
        track.rail_count,
        triggers.iter().count(),
        track.trigger_count,
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
        car.handling_state.label(),
        car.slip_angle.to_degrees(),
        car.ground_source.label(),
        car.current_surface.label(),
        car.wheel_contacts.summary(),
        car.wheel_contacts.split_surface(),
        car.throttle,
        car.steer,
        params.lateral_grip,
        params.acceleration_multiplier,
    );
}

fn toggle_debug_overlay(keys: Res<ButtonInput<KeyCode>>, mut debug: ResMut<DebugOverlayState>) {
    if keys.just_pressed(KeyCode::F3) {
        debug.visible = !debug.visible;
    }
}
