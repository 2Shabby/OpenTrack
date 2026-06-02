use bevy::prelude::*;

use super::generation::{
    GeneratedTrackInfo, RAIL_HEIGHT, RAIL_THICKNESS, TRACK_WIDTH, TrackPiece, TrackPieceKind,
    TrackRecipe, car_spawn_for, generate_track_pieces, validate_piece_connections,
};
use super::markers::{
    GeneratedRail, GeneratedRoadSurface, GeneratedTrigger, SpawnedCamera, SpawnedLighting,
    SpawnedPlayer, SpawnedSceneEntity,
};
use super::scenery::{spawn_forest_scenery, spawn_grass_field};
use crate::car_asset::sports_car_mesh;
use crate::driving::{CarSpawn, ChaseCamera, PlayerCar};
use crate::physics::RailCollider;
use crate::run::{TrackTrigger, TrackTriggerKind};
use crate::spatial::{OrientedRect, Pose2, forward_3d};
use crate::surface::{SurfaceKind, SurfaceZone};

pub fn spawn_generated_track(
    mut commands: Commands,
    recipe: &TrackRecipe,
    asset_server: &AssetServer,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_grass_field(&mut commands, &mut meshes, &mut materials);
    spawn_forest_scenery(&mut commands, asset_server, &mut meshes, &mut materials);

    let pieces = generate_track_pieces(recipe);
    if let Err(error) = validate_piece_connections(&pieces) {
        warn!("generated track connection validation failed: {error}");
    }
    let track_info = GeneratedTrackInfo::from_pieces(recipe, &pieces);
    let car_spawn = car_spawn_for(&pieces);

    for piece in pieces.iter().copied() {
        spawn_piece(&mut commands, &mut meshes, &mut materials, piece);
    }

    commands.insert_resource(track_info);
    commands.insert_resource(car_spawn);

    spawn_car(&mut commands, &mut meshes, &mut materials, car_spawn);
    spawn_lighting(&mut commands);
    spawn_camera(&mut commands, car_spawn);
}

fn spawn_piece(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    piece: TrackPiece,
) {
    let piece_pose = piece.pose();

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(TRACK_WIDTH, piece.length()))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: surface_color(piece.surface),
            perceptual_roughness: 0.92,
            ..default()
        })),
        piece_pose.transform(),
        SurfaceZone::new(piece.surface, piece_pose, piece.bounds()),
        GeneratedRoadSurface,
        SpawnedSceneEntity,
    ));

    spawn_rails(commands, meshes, materials, piece);
    spawn_trigger(commands, meshes, materials, piece);
}

fn spawn_rails(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    piece: TrackPiece,
) {
    let rail_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.12, 0.12),
        perceptual_roughness: 0.7,
        ..default()
    });

    let piece_pose = piece.pose();

    for side in [-1.0, 1.0] {
        let local = Vec2::new(side * (TRACK_WIDTH * 0.5 + RAIL_THICKNESS * 0.5), 0.0);
        let rail_pose = Pose2::new(piece_pose.local_to_world(local), piece_pose.yaw);

        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(RAIL_THICKNESS, RAIL_HEIGHT, piece.length()))),
            MeshMaterial3d(rail_material.clone()),
            Transform::from_xyz(
                rail_pose.position.x,
                RAIL_HEIGHT * 0.5,
                rail_pose.position.y,
            )
            .with_rotation(Quat::from_rotation_y(piece_pose.yaw)),
            RailCollider {
                bounds: OrientedRect::new(
                    rail_pose,
                    Vec2::new(RAIL_THICKNESS * 0.5, piece.length() * 0.5),
                ),
            },
            GeneratedRail,
            SpawnedSceneEntity,
        ));
    }
}

fn spawn_trigger(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    piece: TrackPiece,
) {
    let Some((kind, color, line)) = trigger_for_piece(piece) else {
        return;
    };

    let half_extents = Vec2::new(TRACK_WIDTH * 0.5, 0.45);

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
        Transform::from_xyz(line.position.x, 0.04, line.position.y)
            .with_rotation(Quat::from_rotation_y(line.yaw)),
        TrackTrigger::new(kind, line, half_extents),
        GeneratedTrigger,
        SpawnedSceneEntity,
    ));
}

fn spawn_car(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    car_spawn: CarSpawn,
) {
    commands.spawn((
        Mesh3d(meshes.add(sports_car_mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.08, 0.05),
            metallic: 0.15,
            perceptual_roughness: 0.45,
            ..default()
        })),
        Transform::from_translation(car_spawn.translation)
            .with_rotation(Quat::from_rotation_y(car_spawn.yaw)),
        PlayerCar::default(),
        SpawnedPlayer,
        SpawnedSceneEntity,
    ));
}

fn spawn_lighting(commands: &mut Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 7_500.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.8, 0.0)),
        SpawnedLighting,
        SpawnedSceneEntity,
    ));
}

fn spawn_camera(commands: &mut Commands, car_spawn: CarSpawn) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(
            car_spawn.translation - forward_3d(car_spawn.yaw) * 6.0 + Vec3::Y * 6.5,
        )
        .looking_at(car_spawn.translation, Vec3::Y),
        ChaseCamera,
        SpawnedCamera,
        SpawnedSceneEntity,
    ));
}

fn trigger_for_piece(piece: TrackPiece) -> Option<(TrackTriggerKind, Color, Pose2)> {
    match piece.kind {
        TrackPieceKind::Straight => None,
        TrackPieceKind::Checkpoint(index) => Some((
            TrackTriggerKind::Checkpoint(index),
            Color::srgb(0.15, 0.48, 1.0),
            piece.entry,
        )),
        TrackPieceKind::Finish => Some((
            TrackTriggerKind::Finish,
            Color::srgb(1.0, 1.0, 1.0),
            piece.exit,
        )),
    }
}

fn surface_color(surface: SurfaceKind) -> Color {
    match surface {
        SurfaceKind::Asphalt => Color::srgb(0.19, 0.21, 0.2),
        SurfaceKind::Dirt => Color::srgb(0.45, 0.29, 0.16),
        SurfaceKind::Ice => Color::srgb(0.62, 0.85, 0.9),
        SurfaceKind::Boost => Color::srgb(0.95, 0.67, 0.12),
    }
}
