use bevy::prelude::*;

use super::generation::{
    GeneratedTrackInfo, RAIL_HEIGHT, RAIL_THICKNESS, TRACK_WIDTH, TrackPiece, TrackPieceKind,
    TrackRecipe, car_spawn_for, generate_track_pieces, validate_track_pieces,
};
use super::markers::{
    GeneratedRail, GeneratedRoadSurface, GeneratedTrigger, SpawnedCamera, SpawnedLighting,
    SpawnedPlayer, SpawnedSceneEntity,
};
use super::path_geometry::{line_segments, road_edges};
use super::piece::{TrackPieceMarker, TrackRailSpan, TrackTriggerLine};
use super::road_mesh::road_surface_mesh;
use crate::car_asset::VehicleSelection;
use crate::driving::{CarSpawn, ChaseCamera, PlayerCar, VehicleSceneRoot};
use crate::geometry::forward_3d;
use crate::physics::{
    RailCollider, RoadCollider, VehicleCollider, rail_collision_layers, rail_path_collider,
    road_collision_layers, road_mesh_collider, static_rigid_body, vehicle_collider,
    vehicle_collision_layers, vehicle_rigid_body,
};
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
    validate_track_pieces(&pieces).expect("generated track must validate before spawning");
    let track_info = GeneratedTrackInfo::from_pieces(recipe, &pieces);
    let car_spawn = car_spawn_for(&pieces);
    log_generated_track_debug(recipe, &pieces, car_spawn);

    for piece in pieces.iter() {
        spawn_piece(&mut commands, &mut meshes, &mut materials, piece);
    }
    spawn_track_rails(&mut commands, &mut meshes, &mut materials, &pieces);

    commands.insert_resource(track_info);
    commands.insert_resource(car_spawn);

    spawn_car(&mut commands, asset_server, car_spawn, *vehicle_selection);
    spawn_lighting(&mut commands);
    spawn_camera(&mut commands, car_spawn);
}

fn log_generated_track_debug(recipe: &TrackRecipe, pieces: &[TrackPiece], car_spawn: CarSpawn) {
    let banked_pieces = pieces
        .iter()
        .filter(|piece| {
            matches!(
                piece.kind,
                TrackPieceKind::BankTransition { .. }
                    | TrackPieceKind::BankedStraight { .. }
                    | TrackPieceKind::BankedDoubleStraight { .. }
                    | TrackPieceKind::BankedTurn { .. }
            )
        })
        .count();
    let max_bank = pieces
        .iter()
        .flat_map(|piece| piece.frames.iter())
        .map(|frame| frame.bank.abs())
        .fold(0.0, f32::max);

    info!(
        target: "track_debug",
        "generated seed={} pieces={} banked_pieces={} max_bank_deg={:.1} car_spawn=({:+.2},{:+.2},{:+.2}) yaw={:+.3}",
        recipe.seed,
        pieces.len(),
        banked_pieces,
        max_bank.to_degrees(),
        car_spawn.translation.x,
        car_spawn.translation.y,
        car_spawn.translation.z,
        car_spawn.yaw,
    );
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

    let geometry = piece.geometry();

    spawn_road_surface(commands, meshes, piece, road_material);
    spawn_road_collider(commands, piece);

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

fn spawn_road_collider(commands: &mut Commands, piece: &TrackPiece) {
    let (vertices, indices) = road_collider_mesh(&piece.frames);

    commands.spawn((
        Transform::default(),
        RoadCollider {
            surface: piece.surface,
            boost_direction: piece_boost_direction(piece),
        },
        static_rigid_body(),
        road_collision_layers(),
        road_mesh_collider(vertices, indices),
        SpawnedSceneEntity,
    ));
}

fn piece_boost_direction(piece: &TrackPiece) -> Option<Vec3> {
    (piece.surface == SurfaceKind::Boost).then(|| {
        piece
            .frames
            .get(piece.frames.len() / 2)
            .map(|frame| frame.forward)
            .unwrap_or_else(|| forward_3d(piece.entry().yaw))
    })
}

fn spawn_rail_span(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    rail: TrackRailSpan,
    rail_material: Handle<StandardMaterial>,
) {
    spawn_rail_collider(commands, &rail);

    for segment in line_segments(&rail.points) {
        spawn_rail_segment_visual(commands, meshes, segment, rail_material.clone());
    }
}

fn spawn_track_rails(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pieces: &[TrackPiece],
) {
    let rail_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.12, 0.12),
        perceptual_roughness: 0.7,
        ..default()
    });

    for rail in track_rail_paths(pieces) {
        spawn_rail_span(commands, meshes, rail, rail_material.clone());
    }
}

fn track_rail_paths(pieces: &[TrackPiece]) -> Vec<TrackRailSpan> {
    let mut left = Vec::new();
    let mut right = Vec::new();

    for piece in pieces {
        let edges = road_edges(&piece.frames, TRACK_WIDTH + RAIL_THICKNESS);
        let rail_lift = RAIL_HEIGHT * 0.5;
        append_connected_points(
            &mut left,
            edges
                .left
                .into_iter()
                .zip(&piece.frames)
                .map(|(point, frame)| point + frame.normal * rail_lift)
                .collect(),
        );
        append_connected_points(
            &mut right,
            edges
                .right
                .into_iter()
                .zip(&piece.frames)
                .map(|(point, frame)| point + frame.normal * rail_lift)
                .collect(),
        );
    }

    [left, right]
        .into_iter()
        .filter(|points| points.len() >= 2)
        .map(|points| TrackRailSpan { points })
        .collect()
}

fn append_connected_points(path: &mut Vec<Vec3>, points: Vec<Vec3>) {
    for point in points {
        let duplicate_seam = path
            .last()
            .is_some_and(|previous| previous.distance(point) <= 0.001);
        if !duplicate_seam {
            path.push(point);
        }
    }
}

fn spawn_rail_collider(commands: &mut Commands, rail: &TrackRailSpan) {
    let points = rail.points.clone();

    commands.spawn((
        Transform::default(),
        RailCollider,
        static_rigid_body(),
        rail_collision_layers(),
        rail_path_collider(&points, RAIL_THICKNESS * 0.5),
        GeneratedRail,
        SpawnedSceneEntity,
    ));
}

fn spawn_rail_segment_visual(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    segment: [Vec3; 2],
    rail_material: Handle<StandardMaterial>,
) {
    let [start, end] = segment;
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        return;
    }
    let forward = delta / length;
    let mut up = Vec3::Y;
    if forward.dot(up).abs() > 0.98 {
        up = Vec3::Z;
    }
    let right = up.cross(forward).normalize();
    let up = forward.cross(right).normalize();
    let rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(RAIL_THICKNESS, RAIL_HEIGHT, length))),
        MeshMaterial3d(rail_material),
        Transform::from_translation((start + end) * 0.5).with_rotation(rotation),
        SpawnedSceneEntity,
    ));
}

fn road_collider_mesh(frames: &[super::generation::PathFrame]) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let edges = road_edges(frames, TRACK_WIDTH);
    let mut vertices = Vec::with_capacity(frames.len() * 2);
    let mut indices = Vec::with_capacity(frames.len().saturating_sub(1) * 2);

    for (left, right) in edges.left.into_iter().zip(edges.right) {
        vertices.push(left);
        vertices.push(right);
    }

    for segment in 0..frames.len().saturating_sub(1) {
        let left = (segment * 2) as u32;
        let right = left + 1;
        let next_left = left + 2;
        let next_right = left + 3;
        indices.push([left, next_left, right]);
        indices.push([right, next_left, next_right]);
    }

    (vertices, indices)
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
    let color = trigger_color(trigger.marker);
    let bounds = trigger.bounds;
    let frame = trigger.frame;
    let height = 0.08;

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            bounds.half_extents.x * 2.0,
            height,
            bounds.half_extents.y * 2.0,
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            emissive: color.into(),
            ..default()
        })),
        frame.surface_transform(height * 0.5),
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
        car_spawn.transform(),
        PlayerCar::default(),
        vehicle_rigid_body(),
        vehicle_collision_layers(),
        vehicle_collider(),
        VehicleCollider,
        SpawnedPlayer,
        SpawnedSceneEntity,
    ));

    commands.spawn((
        SceneRoot(asset_server.load(vehicle_selection.fbx_scene_path())),
        car_spawn.transform(),
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
    let rotation = car_spawn.rotation();
    let forward = rotation * Vec3::Z;
    let up = rotation * Vec3::Y;
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(car_spawn.translation - forward * 6.0 + up * 6.5)
            .looking_at(car_spawn.translation, up),
        ChaseCamera,
        SpawnedCamera,
        SpawnedSceneEntity,
    ));
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::track::generation::{PathFrame, TrackPieceKind};

    #[test]
    fn road_collider_mesh_accepts_ground_raycasts() {
        let frames = [
            PathFrame::new(Vec2::ZERO, 0.0, 0.0),
            PathFrame::new(Vec2::new(0.0, 10.0), 0.0, 0.0),
        ];
        let (vertices, indices) = road_collider_mesh(&frames);
        let collider = road_mesh_collider(vertices, indices);

        let hit = collider.cast_ray(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::new(0.0, 2.0, 5.0),
            Vec3::NEG_Y,
            4.0,
            false,
        );

        assert!(hit.is_some());
    }

    #[test]
    fn track_rail_paths_merge_piece_seams() {
        let first = TrackPiece {
            kind: TrackPieceKind::Straight,
            surface: SurfaceKind::Asphalt,
            frames: vec![
                PathFrame::new(Vec2::ZERO, 0.0, 0.0),
                PathFrame::new(Vec2::new(0.0, 10.0), 0.0, 0.0),
            ],
        };
        let second = TrackPiece {
            kind: TrackPieceKind::Straight,
            surface: SurfaceKind::Asphalt,
            frames: vec![
                PathFrame::new(Vec2::new(0.0, 10.0), 0.0, 0.0),
                PathFrame::new(Vec2::new(0.0, 20.0), 0.0, 0.0),
            ],
        };

        let rails = track_rail_paths(&[first, second]);

        assert_eq!(rails.len(), 2);
        assert!(rails.iter().all(|rail| rail.points.len() == 3));
    }
}
