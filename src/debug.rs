use bevy::prelude::*;

use crate::driving::PlayerCar;
use crate::run::RunState;
use crate::surface::SurfaceLibrary;

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
    surfaces: Res<SurfaceLibrary>,
    mut overlay: Single<&mut Text, With<DebugOverlay>>,
) {
    let params = surfaces.get(car.current_surface);

    overlay.0 = format!(
        "time: {:>6.2}\nrun: {}\ncheckpoint: {}/{}\nspeed: {:>5.1}\nsurface: {}\nthrottle: {:+.0}\nsteer: {:+.0}\nlat grip: {:.2}\naccel mult: {:.2}",
        run.elapsed,
        run.status_label(),
        run.next_checkpoint,
        run.checkpoint_count,
        car.velocity.length(),
        car.current_surface.label(),
        car.throttle,
        car.steer,
        params.lateral_grip,
        params.acceleration_multiplier,
    );
}
