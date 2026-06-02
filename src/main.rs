mod debug;
mod driving;
mod run;
mod surface;

use bevy::prelude::*;
use debug::DebugPlugin;
use driving::{CAR_START, DrivingPlugin, PlayerCar};
use run::{RunPlugin, TrackTrigger, TrackTriggerKind};
use surface::{SurfaceKind, SurfacePlugin, SurfaceZone};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.055)))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Open Track Turbo - Surface Sandbox".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((SurfacePlugin, DrivingPlugin, RunPlugin, DebugPlugin))
        .add_systems(Startup, setup_sandbox)
        .run();
}

fn setup_sandbox(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_surface_strip(
        &mut commands,
        &mut meshes,
        &mut materials,
        SurfaceKind::Asphalt,
        -18.0,
        Color::srgb(0.19, 0.21, 0.2),
    );
    spawn_surface_strip(
        &mut commands,
        &mut meshes,
        &mut materials,
        SurfaceKind::Dirt,
        -9.0,
        Color::srgb(0.45, 0.29, 0.16),
    );
    spawn_surface_strip(
        &mut commands,
        &mut meshes,
        &mut materials,
        SurfaceKind::Ice,
        0.0,
        Color::srgb(0.62, 0.85, 0.9),
    );
    spawn_surface_strip(
        &mut commands,
        &mut meshes,
        &mut materials,
        SurfaceKind::Slowdown,
        9.0,
        Color::srgb(0.22, 0.43, 0.18),
    );
    spawn_surface_strip(
        &mut commands,
        &mut meshes,
        &mut materials,
        SurfaceKind::Boost,
        18.0,
        Color::srgb(0.95, 0.67, 0.12),
    );
    spawn_track_trigger(
        &mut commands,
        &mut meshes,
        &mut materials,
        TrackTriggerKind::Checkpoint(0),
        -2.0,
        Color::srgb(0.15, 0.48, 1.0),
    );
    spawn_track_trigger(
        &mut commands,
        &mut meshes,
        &mut materials,
        TrackTriggerKind::Finish,
        22.0,
        Color::srgb(1.0, 1.0, 1.0),
    );

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.35, 0.55, 2.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.13, 0.08),
            ..default()
        })),
        Transform::from_translation(CAR_START),
        PlayerCar::default(),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 7_500.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.8, 0.0)),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 6.5, -22.0).looking_at(CAR_START, Vec3::Y),
        driving::ChaseCamera,
    ));
}

fn spawn_track_trigger(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    kind: TrackTriggerKind,
    z: f32,
    color: Color,
) {
    let half_extents = Vec2::new(24.0, 0.45);

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            half_extents.x * 2.0,
            0.08,
            half_extents.y * 2.0,
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            emissive: color.into(),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.04, z),
        TrackTrigger {
            kind,
            center: Vec2::new(0.0, z),
            half_extents,
        },
    ));
}

fn spawn_surface_strip(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    surface: SurfaceKind,
    x: f32,
    color: Color,
) {
    let half_extents = Vec2::new(4.5, 40.0);

    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(half_extents.x * 2.0, half_extents.y * 2.0),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.92,
            ..default()
        })),
        Transform::from_xyz(x, 0.0, 0.0),
        SurfaceZone {
            kind: surface,
            center: Vec2::new(x, 0.0),
            half_extents,
        },
    ));
}
