mod car_asset;
mod debug;
mod driving;
mod game_state;
mod geometry;
mod hotseat;
mod physics;
mod shell;
mod surface;
mod track;

use bevy::prelude::*;
use bevy_ufbx::FbxPlugin;
use car_asset::CarAssetPlugin;
use debug::DebugPlugin;
use driving::DrivingPlugin;
use hotseat::HotseatPlugin;
use physics::PhysicsQueriesPlugin;
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
        .add_plugins(FbxPlugin)
        .init_state::<game_state::GameState>()
        .add_plugins((
            SurfacePlugin,
            CarAssetPlugin,
            PhysicsQueriesPlugin,
            TrackPlugin,
            DrivingPlugin,
            HotseatPlugin,
            DebugPlugin,
            ShellPlugin,
        ))
        .run();
}
