use bevy::prelude::*;

use super::generation::{
    GeneratedTrackInfo, RAIL_HEIGHT, RAIL_THICKNESS, TRACK_WIDTH, TrackBounds, TrackPiece,
    TrackRecipe, car_spawn_for, generate_track_pieces, validate_track_pieces,
};
use super::markers::{
    GeneratedRail, GeneratedRoadSurface, GeneratedTrigger, SpawnedCamera, SpawnedLighting,
    SpawnedPlayer, SpawnedSceneEntity,
};
use super::piece::{TrackPieceMarker, TrackRailSpan, TrackRoadSpan, TrackTriggerLine};
use super::road_mesh::road_surface_mesh;
use super::scenery::{spawn_forest_scenery, spawn_grass_field};
use crate::car_asset::VehicleSelection;
use crate::driving::{CarSpawn, ChaseCamera, PlayerCar, VehicleSceneRoot};
use crate::geometry::{forward_3d, xz_translation, yaw_rotation};
use crate::physics::{
    RailCollider, RoadCollider, rail_collider, rail_collision_layers, road_collider,
    road_collision_layers, static_rigid_body,
};
use crate::run::{TrackTrigger, TrackTriggerKind};
use crate::surface::SurfaceKind;

pub fn spawn_generated_track(
    mut commands: Commands,
    recipe: &TrackRecipe,
    asset_server: &AssetServer,
    vehicle_selection: &VehicleSelection,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let pieces = generate_track_pieces(recipe);
    if let Err(error) = validate_track_pieces(&pieces) {
        warn!("generated track validation failed: {error}");
    }
    let track_bounds = TrackBounds::from_pieces(&pieces);
    let track_info = GeneratedTrackInfo::from_pieces(recipe, &pieces);
    let car_spawn = car_spawn_for(&pieces);

    spawn_grass_field(&mut commands, &mut meshes, &mut materials, track_bounds);
    spawn_forest_scenery(
        &mut commands,
        asset_server,
        &mut meshes,
        &mut materials,
        track_bounds,
    );

    for piece in pieces.iter() {
        spawn_piece(&mut commands, &mut meshes, &mut materials, piece);
    }

    commands.insert_resource(track_info);
    commands.insert_resource(car_spawn);

    spawn_car(&mut commands, asset_server, car_spawn, *vehicle_selection);
    spawn_lighting(&mut commands);
    spawn_camera(&mut commands, car_spawn);
}

fn spawn_piece(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    piece: &TrackPiece,
) {
    let road_material = materials.add(StandardMaterial {
        base_color: surface_color(piece.surface),
        perceptual_roughness: 0.92,
        ..default()
    });
    let rail_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.12, 0.12),
        perceptual_roughness: 0.7,
        ..default()
    });

    let geometry = piece.geometry();

    spawn_road_surface(commands, meshes, piece, road_material);

    for road in geometry.roads {
        spawn_road_collider(commands, road);
    }

    for rail in geometry.rails {
        spawn_rail_span(commands, meshes, rail, rail_material.clone());
    }

    spawn_trigger(commands, meshes, materials, geometry.trigger);
}

fn spawn_road_surface(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    piece: &TrackPiece,
    road_material: Handle<StandardMaterial>,
) {
    commands.spawn((
        Mesh3d(meshes.add(road_surface_mesh(&piece.frames, TRACK_WIDTH))),
        MeshMaterial3d(road_material),
        Transform::default(),
        GeneratedRoadSurface,
        SpawnedSceneEntity,
    ));
}

fn spawn_road_collider(commands: &mut Commands, road: TrackRoadSpan) {
    commands.spawn((
        Transform::from_translation(xz_translation(road.bounds.pose.position, -0.04))
            .with_rotation(yaw_rotation(road.bounds.pose.yaw)),
        RoadCollider {
            surface: road.surface,
        },
        static_rigid_body(),
        road_collision_layers(),
        road_collider(TRACK_WIDTH, 0.08, road.length),
        SpawnedSceneEntity,
    ));
}

fn spawn_rail_span(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    rail: TrackRailSpan,
    rail_material: Handle<StandardMaterial>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(RAIL_THICKNESS, RAIL_HEIGHT, rail.length))),
        MeshMaterial3d(rail_material),
        Transform::from_translation(xz_translation(rail.bounds.pose.position, RAIL_HEIGHT * 0.5))
            .with_rotation(yaw_rotation(rail.bounds.pose.yaw)),
        RailCollider {
            bounds: rail.bounds,
        },
        static_rigid_body(),
        rail_collision_layers(),
        rail_collider(RAIL_THICKNESS, RAIL_HEIGHT, rail.length),
        GeneratedRail,
        SpawnedSceneEntity,
    ));
}

fn spawn_trigger(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    trigger: Option<TrackTriggerLine>,
) {
    let Some(trigger) = trigger else {
        return;
    };
    let kind = trigger_kind(trigger.marker);
    let color = trigger_color(trigger.marker);
    let bounds = trigger.bounds;

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            bounds.half_extents.x * 2.0,
            0.08,
            bounds.half_extents.y * 2.0,
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            emissive: color.into(),
            ..default()
        })),
        Transform::from_translation(xz_translation(bounds.pose.position, 0.04))
            .with_rotation(yaw_rotation(bounds.pose.yaw)),
        TrackTrigger { kind, bounds },
        GeneratedTrigger,
        SpawnedSceneEntity,
    ));
}

fn spawn_car(
    commands: &mut Commands,
    asset_server: &AssetServer,
    car_spawn: CarSpawn,
    vehicle_selection: VehicleSelection,
) {
    commands.spawn((
        Transform::from_translation(car_spawn.translation)
            .with_rotation(yaw_rotation(car_spawn.yaw)),
        PlayerCar::default(),
        SpawnedPlayer,
        SpawnedSceneEntity,
    ));

    commands.spawn((
        SceneRoot(asset_server.load(vehicle_selection.vehicle.fbx_scene_path())),
        Transform::from_translation(car_spawn.translation)
            .with_rotation(yaw_rotation(car_spawn.yaw)),
        VehicleSceneRoot,
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

fn trigger_kind(marker: TrackPieceMarker) -> TrackTriggerKind {
    match marker {
        TrackPieceMarker::Checkpoint(index) => TrackTriggerKind::Checkpoint(index),
        TrackPieceMarker::Finish => TrackTriggerKind::Finish,
    }
}

fn trigger_color(marker: TrackPieceMarker) -> Color {
    match marker {
        TrackPieceMarker::Checkpoint(_) => Color::srgb(0.15, 0.48, 1.0),
        TrackPieceMarker::Finish => Color::srgb(1.0, 1.0, 1.0),
    }
}

fn surface_color(surface: SurfaceKind) -> Color {
    match surface {
        SurfaceKind::Asphalt => Color::srgb(0.19, 0.21, 0.2),
        SurfaceKind::Dirt => Color::srgb(0.45, 0.29, 0.16),
        SurfaceKind::Ice => Color::srgb(0.62, 0.85, 0.9),
        SurfaceKind::Boost => Color::srgb(0.95, 0.67, 0.12),
        SurfaceKind::Grass => Color::srgb(0.16, 0.34, 0.13),
    }
}
