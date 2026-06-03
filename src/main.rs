mod car_asset;
mod debug;
mod driving;
mod game_state;
mod geometry;
mod ghost;
mod hotseat;
mod hud;
mod physics;
mod run;
mod shell;
mod surface;
mod track;

use bevy::prelude::*;
use debug::DebugPlugin;
use driving::DrivingPlugin;
use ghost::GhostPlugin;
use hotseat::HotseatPlugin;
use hud::HudPlugin;
use physics::PhysicsQueriesPlugin;
use run::RunPlugin;
use shell::ShellPlugin;
use surface::SurfacePlugin;
use track::TrackPlugin;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.055)))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Open Track Turbo".to_string(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<game_state::GameState>()
        .add_plugins((
            SurfacePlugin,
            PhysicsQueriesPlugin,
            TrackPlugin,
            DrivingPlugin,
            RunPlugin,
            HotseatPlugin,
            GhostPlugin,
            HudPlugin,
            DebugPlugin,
            ShellPlugin,
        ))
        .run();
}
