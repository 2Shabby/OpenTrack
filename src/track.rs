use bevy::prelude::*;

use crate::car_asset::sports_car_mesh;
use crate::driving::{CAR_START, ChaseCamera, PlayerCar};
use crate::physics::RailCollider;
use crate::run::{TrackTrigger, TrackTriggerKind};
use crate::surface::{SurfaceKind, SurfaceZone};

const TRACK_WIDTH: f32 = 12.0;
const PIECE_LENGTH: f32 = 14.0;
const RAIL_HEIGHT: f32 = 0.45;
const RAIL_THICKNESS: f32 = 0.28;

pub struct TrackPlugin;

impl Plugin for TrackPlugin {
    fn build(&self, _app: &mut App) {}
}

#[derive(Clone, Copy, Debug)]
pub enum TrackPieceKind {
    Straight,
    Checkpoint(usize),
    Finish,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackPieceDefinition {
    pub kind: TrackPieceKind,
    pub surface: SurfaceKind,
    pub center: Vec3,
    pub yaw: f32,
}

impl TrackPieceDefinition {
    fn bounds(self) -> Vec2 {
        Vec2::new(TRACK_WIDTH * 0.5, PIECE_LENGTH * 0.5)
    }
}

pub fn spawn_sandbox_track(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_grass_field(&mut commands, &mut meshes, &mut materials);
    spawn_forest_scenery(&mut commands, &asset_server, &mut meshes, &mut materials);

    let pieces = [
        TrackPieceDefinition {
            kind: TrackPieceKind::Straight,
            surface: SurfaceKind::Asphalt,
            center: Vec3::new(0.0, 0.0, -21.0),
            yaw: 0.0,
        },
        TrackPieceDefinition {
            kind: TrackPieceKind::Checkpoint(0),
            surface: SurfaceKind::Dirt,
            center: Vec3::new(0.0, 0.0, -7.0),
            yaw: 0.0,
        },
        TrackPieceDefinition {
            kind: TrackPieceKind::Straight,
            surface: SurfaceKind::Ice,
            center: Vec3::new(0.0, 0.0, 7.0),
            yaw: 0.0,
        },
        TrackPieceDefinition {
            kind: TrackPieceKind::Finish,
            surface: SurfaceKind::Boost,
            center: Vec3::new(0.0, 0.0, 21.0),
            yaw: 0.0,
        },
    ];

    for piece in pieces {
        spawn_piece(&mut commands, &mut meshes, &mut materials, piece);
    }

    spawn_car(&mut commands, &mut meshes, &mut materials);
    spawn_lighting(&mut commands);
    spawn_camera(&mut commands);
}

fn spawn_piece(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    piece: TrackPieceDefinition,
) {
    let bounds = piece.bounds();
    let road_color = surface_color(piece.surface);

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(TRACK_WIDTH, PIECE_LENGTH))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: road_color,
            perceptual_roughness: 0.92,
            ..default()
        })),
        Transform::from_translation(piece.center).with_rotation(Quat::from_rotation_y(piece.yaw)),
        SurfaceZone {
            kind: piece.surface,
            center: Vec2::new(piece.center.x, piece.center.z),
            half_extents: bounds,
        },
    ));

    spawn_rails(commands, meshes, materials, piece);
    spawn_trigger(commands, meshes, materials, piece);
}

fn spawn_rails(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    piece: TrackPieceDefinition,
) {
    let rail_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.12, 0.12),
        perceptual_roughness: 0.7,
        ..default()
    });

    for side in [-1.0, 1.0] {
        let center = Vec2::new(
            side * (TRACK_WIDTH * 0.5 + RAIL_THICKNESS * 0.5),
            piece.center.z,
        );

        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(RAIL_THICKNESS, RAIL_HEIGHT, PIECE_LENGTH))),
            MeshMaterial3d(rail_material.clone()),
            Transform::from_xyz(center.x, RAIL_HEIGHT * 0.5, center.y),
            RailCollider {
                center,
                half_extents: Vec2::new(RAIL_THICKNESS * 0.5, PIECE_LENGTH * 0.5),
            },
        ));
    }
}

fn spawn_trigger(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    piece: TrackPieceDefinition,
) {
    let Some((kind, color)) = trigger_for_piece(piece.kind) else {
        return;
    };

    let half_extents = Vec2::new(TRACK_WIDTH * 0.5, 0.45);
    let z = piece.center.z + PIECE_LENGTH * 0.4;

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
        Transform::from_xyz(piece.center.x, 0.04, z),
        TrackTrigger {
            kind,
            center: Vec2::new(piece.center.x, z),
            half_extents,
        },
    ));
}

fn spawn_car(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.spawn((
        Mesh3d(meshes.add(sports_car_mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.08, 0.05),
            metallic: 0.15,
            perceptual_roughness: 0.45,
            ..default()
        })),
        Transform::from_translation(CAR_START),
        PlayerCar::default(),
    ));
}

fn spawn_grass_field(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(90.0, 90.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.34, 0.13),
            perceptual_roughness: 0.98,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.015, 0.0),
    ));
}

fn spawn_forest_scenery(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let oak_leaf = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.58, 0.18),
        base_color_texture: Some(asset_server.load("forest/OakTreeLeaf.png")),
        perceptual_roughness: 0.9,
        ..default()
    });
    let oak_trunk = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.27, 0.12),
        base_color_texture: Some(asset_server.load("forest/OakTreeTrunk.png")),
        perceptual_roughness: 0.85,
        ..default()
    });
    let spruce_leaf = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.43, 0.2),
        base_color_texture: Some(asset_server.load("forest/SpruceTreeLeaf.png")),
        perceptual_roughness: 0.9,
        ..default()
    });
    let spruce_trunk = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.25, 0.11),
        base_color_texture: Some(asset_server.load("forest/SpruceTreeTrunk.png")),
        perceptual_roughness: 0.85,
        ..default()
    });
    let rock = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.43, 0.38),
        base_color_texture: Some(asset_server.load("forest/rockTexture1.png")),
        perceptual_roughness: 0.95,
        ..default()
    });

    let tree_positions = [
        Vec3::new(-18.0, 0.0, -30.0),
        Vec3::new(17.0, 0.0, -28.0),
        Vec3::new(-23.0, 0.0, -18.0),
        Vec3::new(20.0, 0.0, -12.0),
        Vec3::new(-19.0, 0.0, -3.0),
        Vec3::new(24.0, 0.0, 5.0),
        Vec3::new(-22.0, 0.0, 14.0),
        Vec3::new(18.0, 0.0, 23.0),
        Vec3::new(-16.0, 0.0, 31.0),
        Vec3::new(23.0, 0.0, 33.0),
    ];

    for (index, position) in tree_positions.into_iter().enumerate() {
        if index % 2 == 0 {
            spawn_oak_tree(
                commands,
                meshes,
                oak_trunk.clone(),
                oak_leaf.clone(),
                position,
            );
        } else {
            spawn_spruce_tree(
                commands,
                meshes,
                spruce_trunk.clone(),
                spruce_leaf.clone(),
                position,
            );
        }
    }

    for (index, position) in [
        Vec3::new(-13.5, 0.0, -22.0),
        Vec3::new(14.5, 0.0, -4.0),
        Vec3::new(-14.0, 0.0, 10.0),
        Vec3::new(13.5, 0.0, 28.0),
    ]
    .into_iter()
    .enumerate()
    {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.4, 0.7, 1.0))),
            MeshMaterial3d(rock.clone()),
            Transform::from_translation(position + Vec3::Y * 0.35).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                index as f32 * 0.8,
                0.15,
                0.2,
            )),
        ));
    }
}

fn spawn_oak_tree(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    trunk_material: Handle<StandardMaterial>,
    leaf_material: Handle<StandardMaterial>,
    position: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.28, 2.4).mesh().resolution(7))),
        MeshMaterial3d(trunk_material),
        Transform::from_translation(position + Vec3::Y * 1.2),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.45).mesh().ico(1).expect("valid ico sphere"))),
        MeshMaterial3d(leaf_material),
        Transform::from_translation(position + Vec3::Y * 2.65),
    ));
}

fn spawn_spruce_tree(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    trunk_material: Handle<StandardMaterial>,
    leaf_material: Handle<StandardMaterial>,
    position: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.22, 2.1).mesh().resolution(7))),
        MeshMaterial3d(trunk_material),
        Transform::from_translation(position + Vec3::Y * 1.05),
    ));

    for (height, radius, y) in [(1.8, 1.6, 2.0), (1.6, 1.25, 2.75), (1.35, 0.9, 3.35)] {
        commands.spawn((
            Mesh3d(meshes.add(Cone::new(radius, height).mesh().resolution(7))),
            MeshMaterial3d(leaf_material.clone()),
            Transform::from_translation(position + Vec3::Y * y),
        ));
    }
}

fn spawn_lighting(commands: &mut Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 7_500.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.8, 0.0)),
    ));
}

fn spawn_camera(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 6.5, -32.0).looking_at(CAR_START, Vec3::Y),
        ChaseCamera,
    ));
}

fn trigger_for_piece(kind: TrackPieceKind) -> Option<(TrackTriggerKind, Color)> {
    match kind {
        TrackPieceKind::Straight => None,
        TrackPieceKind::Checkpoint(index) => Some((
            TrackTriggerKind::Checkpoint(index),
            Color::srgb(0.15, 0.48, 1.0),
        )),
        TrackPieceKind::Finish => Some((TrackTriggerKind::Finish, Color::srgb(1.0, 1.0, 1.0))),
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
