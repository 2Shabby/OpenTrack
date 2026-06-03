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
    let boost_direction = car.boost_direction.unwrap_or(Vec3::ZERO);

    overlay.0.0 = format!(
        "seed: {}\npieces: {}\ntrack cps: {}\nroad: {}/{}\nrail: {}/{}\ntrigger: {}/{}\nplayer: {}\nplayers: {}\nbest: {}\nghost: {}\ntime: {:>6.2}\nrun: {}\ncheckpoint: {}/{}\nspeed: {:>5.1}\nsigned: {:+5.1}\ntarget: {:+5.1}\nmode: {}\nhandling: {}\ndrift: {} reason: {}\nyaw target/actual: {:+.2}/{:+.2}\ncollision: {}\nmove req/ok: {:>4.2}/{:>4.2}\nyaw req/ok: {:+.2}/{:+.2}\nhits: {} yaw limited: {}\nhit normal: {:+.2},{:+.2},{:+.2}\ndepen: {:>4.2}\nslip: {:>4.0} deg\nground: {} {}\nboost dir: {:+.2},{:+.2},{:+.2}\nwheels: {}\nsplit: {}\nthrottle: {:+.0}\nsteer: {:+.0}\nrear brake: {:+.0}\nrear cost/yaw: {:.2}/{:+.2}\nwheel rpm: {:+5.0}/{:+5.0}/{:+5.0}/{:+5.0}\ntarget rpm: {:+5.0}/{:+5.0}/{:+5.0}/{:+5.0}\nwheel slip: {:+.2}/{:+.2}/{:+.2}/{:+.2}\nsusp comp: {:.2}/{:.2}/{:.2}/{:.2}\nsusp off: {:+.2}/{:+.2}/{:+.2}/{:+.2}\nload all/f/r: {:>5.0}/{:>5.0}/{:>5.0}\nload wheels: {:>4.0}/{:>4.0}/{:>4.0}/{:>4.0}\nfriction: {:>6.0}\nlong all/f/r: {:+6.0}/{:+6.0}/{:+6.0}\nlong wheels: {:+5.0}/{:+5.0}/{:+5.0}/{:+5.0}\nlat limit f/r: {:>6.0}/{:>6.0}\nlat limit wheels: {:>4.0}/{:>4.0}/{:>4.0}/{:>4.0}\nlat all/f/r: {:+6.0}/{:+6.0}/{:+6.0}\nlat wheels: {:+5.0}/{:+5.0}/{:+5.0}/{:+5.0}\nsat f/r/all: {:.2}/{:.2}/{:.2}\nsat wheels: {:.2}/{:.2}/{:.2}/{:.2}",
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
        car.tire_forces.target_speed,
        car.drive_mode.label(),
        car.handling_state.label(),
        car.drift_assist.state.label(),
        car.tire_forces.slide_reason.label(),
        car.tire_forces.target_yaw_rate,
        car.yaw_rate,
        car.collision_state.label(),
        car.collision_debug.requested_translation_delta.length(),
        car.collision_debug.accepted_translation_delta.length(),
        car.collision_debug.requested_yaw_delta,
        car.collision_debug.accepted_yaw_delta,
        car.collision_debug.hit_count,
        car.collision_debug.yaw_limited,
        car.collision_debug.last_hit_normal.x,
        car.collision_debug.last_hit_normal.y,
        car.collision_debug.last_hit_normal.z,
        car.collision_debug.depenetration.length(),
        car.slip_angle.to_degrees(),
        car.ground_source.label(),
        car.current_surface.label(),
        boost_direction.x,
        boost_direction.y,
        boost_direction.z,
        car.wheel_contacts.summary(),
        car.wheel_contacts.split_surface(),
        car.throttle,
        car.steer,
        car.rear_brake,
        car.tire_forces.rear_brake_lateral_cost,
        car.tire_forces.yaw_assist,
        radians_per_second_to_rpm(car.wheel_telemetry[0].angular_speed),
        radians_per_second_to_rpm(car.wheel_telemetry[1].angular_speed),
        radians_per_second_to_rpm(car.wheel_telemetry[2].angular_speed),
        radians_per_second_to_rpm(car.wheel_telemetry[3].angular_speed),
        radians_per_second_to_rpm(car.wheel_telemetry[0].target_angular_speed),
        radians_per_second_to_rpm(car.wheel_telemetry[1].target_angular_speed),
        radians_per_second_to_rpm(car.wheel_telemetry[2].target_angular_speed),
        radians_per_second_to_rpm(car.wheel_telemetry[3].target_angular_speed),
        car.wheel_telemetry[0].slip_ratio,
        car.wheel_telemetry[1].slip_ratio,
        car.wheel_telemetry[2].slip_ratio,
        car.wheel_telemetry[3].slip_ratio,
        car.wheel_suspension[0].compression,
        car.wheel_suspension[1].compression,
        car.wheel_suspension[2].compression,
        car.wheel_suspension[3].compression,
        car.wheel_suspension[0].visual_offset,
        car.wheel_suspension[1].visual_offset,
        car.wheel_suspension[2].visual_offset,
        car.wheel_suspension[3].visual_offset,
        car.tire_forces.normal_load,
        car.tire_forces.front_normal_load,
        car.tire_forces.rear_normal_load,
        car.tire_forces.wheel_normal_loads[0],
        car.tire_forces.wheel_normal_loads[1],
        car.tire_forces.wheel_normal_loads[2],
        car.tire_forces.wheel_normal_loads[3],
        car.tire_forces.friction_limit,
        car.tire_forces.longitudinal_force,
        car.tire_forces.front_longitudinal_force,
        car.tire_forces.rear_longitudinal_force,
        car.tire_forces.wheel_longitudinal_forces[0],
        car.tire_forces.wheel_longitudinal_forces[1],
        car.tire_forces.wheel_longitudinal_forces[2],
        car.tire_forces.wheel_longitudinal_forces[3],
        car.tire_forces.front_lateral_limit,
        car.tire_forces.rear_lateral_limit,
        car.tire_forces.wheel_lateral_limits[0],
        car.tire_forces.wheel_lateral_limits[1],
        car.tire_forces.wheel_lateral_limits[2],
        car.tire_forces.wheel_lateral_limits[3],
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

fn radians_per_second_to_rpm(value: f32) -> f32 {
    value * 60.0 / std::f32::consts::TAU
}

fn toggle_debug_overlay(keys: Res<ButtonInput<KeyCode>>, mut debug: ResMut<DebugOverlayState>) {
    if keys.just_pressed(KeyCode::F3) {
        debug.visible = !debug.visible;
    }
}
