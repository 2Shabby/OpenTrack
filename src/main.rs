mod debug;
mod driving;
mod physics;
mod run;
mod surface;
mod track;

use bevy::prelude::*;
use debug::DebugPlugin;
use driving::DrivingPlugin;
use physics::PhysicsQueriesPlugin;
use run::RunPlugin;
use surface::SurfacePlugin;
use track::{TrackPlugin, spawn_sandbox_track};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.055)))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Open Track Turbo - Piece Sandbox".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            SurfacePlugin,
            PhysicsQueriesPlugin,
            TrackPlugin,
            DrivingPlugin,
            RunPlugin,
            DebugPlugin,
        ))
        .add_systems(Startup, spawn_sandbox_track)
        .run();
}
