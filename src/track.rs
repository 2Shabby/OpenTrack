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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
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
