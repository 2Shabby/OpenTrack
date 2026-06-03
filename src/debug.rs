use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::driving::PlayerCar;
use crate::game_state::GameState;
use crate::ghost::SessionBestGhost;
use crate::hotseat::HotseatSession;
use crate::run::RunState;
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

#[derive(SystemParam)]
struct DebugSnapshot<'w, 's> {
    car: Single<'w, 's, &'static PlayerCar>,
    run: Res<'w, RunState>,
    ghost: Res<'w, SessionBestGhost>,
    hotseat: Res<'w, HotseatSession>,
    track: Res<'w, GeneratedTrackInfo>,
    road_surfaces: Query<'w, 's, (), With<GeneratedRoadSurface>>,
    rails: Query<'w, 's, (), With<GeneratedRail>>,
    triggers: Query<'w, 's, (), With<GeneratedTrigger>>,
}

fn update_debug_overlay(
    debug: Res<DebugOverlayState>,
    snapshot: DebugSnapshot,
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

    let car = *snapshot.car;
    let best = snapshot
        .hotseat
        .best_summary()
        .map(|(name, finish_time)| format!("{name} {finish_time:.2}"))
        .unwrap_or_else(|| "none".to_string());
    let ghost_best = snapshot
        .ghost
        .finish_time()
        .map(|time| format!("{time:.2}"))
        .unwrap_or_else(|| "none".to_string());

    overlay.0.0 = format!(
        "seed: {}\npieces: {}\ntrack cps: {}\nroad: {}/{}\nrail: {}/{}\ntrigger: {}/{}\nplayer: {}\nplayers: {}\nbest: {}\nghost: {}\ntime: {:>6.2}\nrun: {}\ncheckpoint: {}/{}\nspeed: {:>5.1}\nsigned: {:+5.1}\nmode: {}\nhandling: {}\ncollision: {}\nslip: {:>4.0} deg\nground: {} {}\nwheels: {}\nsplit: {}\nthrottle: {:+.0}\nsteer: {:+.0}\nload all/f/r: {:>5.0}/{:>5.0}/{:>5.0}\nload wheels: {:>4.0}/{:>4.0}/{:>4.0}/{:>4.0}\nfriction: {:>6.0}\nlong force: {:+6.0}\nlat all/f/r: {:+6.0}/{:+6.0}/{:+6.0}\nlat wheels: {:+5.0}/{:+5.0}/{:+5.0}/{:+5.0}\nsat f/r/all: {:.2}/{:.2}/{:.2}\nsat wheels: {:.2}/{:.2}/{:.2}/{:.2}",
        snapshot.track.seed,
        snapshot.track.piece_count,
        snapshot.track.checkpoint_count,
        snapshot.road_surfaces.iter().count(),
        snapshot.track.road_surface_count,
        snapshot.rails.iter().count(),
        snapshot.track.rail_count,
        snapshot.triggers.iter().count(),
        snapshot.track.trigger_count,
        snapshot.hotseat.active_player_name(),
        snapshot.hotseat.player_count(),
        best,
        ghost_best,
        snapshot.run.elapsed,
        snapshot.run.status_label(),
        snapshot.run.next_checkpoint,
        snapshot.run.checkpoint_count,
        car.velocity.length(),
        car.signed_speed,
        car.drive_mode.label(),
        car.handling_state.label(),
        car.collision_state.label(),
        car.slip_angle.to_degrees(),
        car.ground_source.label(),
        car.current_surface.label(),
        car.wheel_contacts.summary(),
        car.wheel_contacts.split_surface(),
        car.throttle,
        car.steer,
        car.tire_forces.normal_load,
        car.tire_forces.front_normal_load,
        car.tire_forces.rear_normal_load,
        car.tire_forces.wheel_normal_loads[0],
        car.tire_forces.wheel_normal_loads[1],
        car.tire_forces.wheel_normal_loads[2],
        car.tire_forces.wheel_normal_loads[3],
        car.tire_forces.friction_limit,
        car.tire_forces.longitudinal_force,
        car.tire_forces.lateral_force,
        car.tire_forces.front_lateral_force,
        car.tire_forces.rear_lateral_force,
        car.tire_forces.wheel_lateral_forces[0],
        car.tire_forces.wheel_lateral_forces[1],
        car.tire_forces.wheel_lateral_forces[2],
        car.tire_forces.wheel_lateral_forces[3],
        car.tire_forces.front_saturation,
        car.tire_forces.rear_saturation,
        car.tire_forces.saturation,
        car.tire_forces.wheel_saturations[0],
        car.tire_forces.wheel_saturations[1],
        car.tire_forces.wheel_saturations[2],
        car.tire_forces.wheel_saturations[3],
    );
}

fn toggle_debug_overlay(keys: Res<ButtonInput<KeyCode>>, mut debug: ResMut<DebugOverlayState>) {
    if keys.just_pressed(KeyCode::F3) {
        debug.visible = !debug.visible;
    }
}
