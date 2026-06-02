use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::car_asset::sports_car_mesh;
use crate::driving::{CarSpawn, ChaseCamera, PlayerCar};
use crate::physics::RailCollider;
use crate::run::{TrackTrigger, TrackTriggerKind};
use crate::surface::{SurfaceKind, SurfaceZone};

const TRACK_WIDTH: f32 = 12.0;
const PIECE_LENGTH: f32 = 14.0;
const RAIL_HEIGHT: f32 = 0.45;
const RAIL_THICKNESS: f32 = 0.28;

pub struct TrackPlugin;

impl Plugin for TrackPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TrackRecipe::default());
    }
}

#[derive(Resource)]
pub struct TrackRecipe {
    pub seed: u64,
    pub piece_count: usize,
}

impl Default for TrackRecipe {
    fn default() -> Self {
        Self {
            seed: 0x5EED_2026,
            piece_count: 8,
        }
    }
}

#[derive(Resource)]
pub struct GeneratedTrackInfo {
    pub seed: u64,
    pub piece_count: usize,
    pub checkpoint_count: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum TrackPieceKind {
    Straight,
    Curve,
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
    recipe: Res<TrackRecipe>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_grass_field(&mut commands, &mut meshes, &mut materials);
    spawn_forest_scenery(&mut commands, &asset_server, &mut meshes, &mut materials);

    let pieces = generate_track_pieces(&recipe);
    let checkpoint_count = pieces
        .iter()
        .filter(|piece| matches!(piece.kind, TrackPieceKind::Checkpoint(_)))
        .count();
    let car_spawn = car_spawn_for(&pieces);

    for piece in pieces.iter().copied() {
        spawn_piece(&mut commands, &mut meshes, &mut materials, piece);
    }

    commands.insert_resource(GeneratedTrackInfo {
        seed: recipe.seed,
        piece_count: pieces.len(),
        checkpoint_count,
    });
    commands.insert_resource(car_spawn);

    spawn_car(&mut commands, &mut meshes, &mut materials, car_spawn);
    spawn_lighting(&mut commands);
    spawn_camera(&mut commands, car_spawn);
}

fn generate_track_pieces(recipe: &TrackRecipe) -> Vec<TrackPieceDefinition> {
    let piece_count = recipe.piece_count.max(4);
    let checkpoint_index = piece_count / 2;
    let mut rng = ChaCha8Rng::seed_from_u64(recipe.seed);
    let mut cursor = Vec2::new(0.0, -((piece_count as f32) * PIECE_LENGTH * 0.5));
    let mut yaw = 0.0;

    (0..piece_count)
        .map(|index| {
            if index > 0 && index < piece_count - 1 {
                yaw = (yaw + generated_yaw_delta(&mut rng)).clamp(-0.55, 0.55);
            }

            let forward = forward_2d(yaw);
            let center = cursor + forward * (PIECE_LENGTH * 0.5);
            cursor += forward * PIECE_LENGTH;

            let kind = if index == piece_count - 1 {
                TrackPieceKind::Finish
            } else if index == checkpoint_index {
                TrackPieceKind::Checkpoint(0)
            } else if yaw.abs() > 0.05 {
                TrackPieceKind::Curve
            } else {
                TrackPieceKind::Straight
            };
            let surface = match kind {
                TrackPieceKind::Finish => SurfaceKind::Boost,
                _ => generated_surface(&mut rng),
            };

            TrackPieceDefinition {
                kind,
                surface,
                center: Vec3::new(center.x, 0.0, center.y),
                yaw,
            }
        })
        .collect()
}

fn generated_surface(rng: &mut ChaCha8Rng) -> SurfaceKind {
    match rng.random_range(0..10) {
        0..=5 => SurfaceKind::Asphalt,
        6..=7 => SurfaceKind::Dirt,
        8 => SurfaceKind::Ice,
        _ => SurfaceKind::Boost,
    }
}

fn generated_yaw_delta(rng: &mut ChaCha8Rng) -> f32 {
    match rng.random_range(0..5) {
        0 => -0.18,
        1 => 0.18,
        _ => 0.0,
    }
}

fn car_spawn_for(pieces: &[TrackPieceDefinition]) -> CarSpawn {
    let Some(first_piece) = pieces.first() else {
        return CarSpawn::default();
    };
    let start = Vec2::new(first_piece.center.x, first_piece.center.z)
        - forward_2d(first_piece.yaw) * (PIECE_LENGTH * 0.42);

    CarSpawn {
        translation: Vec3::new(start.x, 0.05, start.y),
        yaw: first_piece.yaw,
    }
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
            yaw: piece.yaw,
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
        let local = Vec2::new(side * (TRACK_WIDTH * 0.5 + RAIL_THICKNESS * 0.5), 0.0);
        let center = Vec2::new(piece.center.x, piece.center.z) + rotate_2d(local, piece.yaw);

        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(RAIL_THICKNESS, RAIL_HEIGHT, PIECE_LENGTH))),
            MeshMaterial3d(rail_material.clone()),
            Transform::from_xyz(center.x, RAIL_HEIGHT * 0.5, center.y)
                .with_rotation(Quat::from_rotation_y(piece.yaw)),
            RailCollider {
                center,
                half_extents: Vec2::new(RAIL_THICKNESS * 0.5, PIECE_LENGTH * 0.5),
                yaw: piece.yaw,
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
    let center = Vec2::new(piece.center.x, piece.center.z)
        + rotate_2d(Vec2::new(0.0, PIECE_LENGTH * 0.4), piece.yaw);

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
        Transform::from_xyz(center.x, 0.04, center.y)
            .with_rotation(Quat::from_rotation_y(piece.yaw)),
        TrackTrigger {
            kind,
            center,
            half_extents,
            yaw: piece.yaw,
        },
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

fn spawn_camera(commands: &mut Commands, car_spawn: CarSpawn) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(
            car_spawn.translation - forward_3d(car_spawn.yaw) * 6.0 + Vec3::Y * 6.5,
        )
        .looking_at(car_spawn.translation, Vec3::Y),
        ChaseCamera,
    ));
}

fn trigger_for_piece(kind: TrackPieceKind) -> Option<(TrackTriggerKind, Color)> {
    match kind {
        TrackPieceKind::Straight | TrackPieceKind::Curve => None,
        TrackPieceKind::Checkpoint(index) => Some((
            TrackTriggerKind::Checkpoint(index),
            Color::srgb(0.15, 0.48, 1.0),
        )),
        TrackPieceKind::Finish => Some((TrackTriggerKind::Finish, Color::srgb(1.0, 1.0, 1.0))),
    }
}

fn forward_2d(yaw: f32) -> Vec2 {
    Vec2::new(yaw.sin(), yaw.cos())
}

fn forward_3d(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

fn rotate_2d(value: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(value.x * cos - value.y * sin, value.x * sin + value.y * cos)
}

fn surface_color(surface: SurfaceKind) -> Color {
    match surface {
        SurfaceKind::Asphalt => Color::srgb(0.19, 0.21, 0.2),
        SurfaceKind::Dirt => Color::srgb(0.45, 0.29, 0.16),
        SurfaceKind::Ice => Color::srgb(0.62, 0.85, 0.9),
        SurfaceKind::Boost => Color::srgb(0.95, 0.67, 0.12),
    }
}
