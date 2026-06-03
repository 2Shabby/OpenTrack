use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::driving::CarSpawn;
use crate::spatial::{OrientedRect, Pose2, rotate_2d};
use crate::surface::SurfaceKind;

pub const TRACK_WIDTH: f32 = 12.0;
pub const PIECE_LENGTH: f32 = 14.0;
pub const RAIL_HEIGHT: f32 = 0.45;
pub const RAIL_THICKNESS: f32 = 0.28;
const MAX_ROUTE_YAW: f32 = std::f32::consts::PI * 0.85;
const ROAD_OVERLAP_SHRINK: f32 = 0.35;

#[derive(Resource)]
pub struct TrackRecipe {
    pub seed: u64,
    pub piece_count: usize,
    pub difficulty: u8,
    pub surface_mix: SurfaceMix,
}

impl Default for TrackRecipe {
    fn default() -> Self {
        Self {
            seed: 0x5EED_2026,
            piece_count: 8,
            difficulty: 1,
            surface_mix: SurfaceMix::Balanced,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceMix {
    Balanced,
    Technical,
    Fast,
}

impl SurfaceMix {
    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "Balanced",
            Self::Technical => "Technical",
            Self::Fast => "Fast",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Balanced => Self::Technical,
            Self::Technical => Self::Fast,
            Self::Fast => Self::Balanced,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Balanced => Self::Fast,
            Self::Technical => Self::Balanced,
            Self::Fast => Self::Technical,
        }
    }
}

#[derive(Resource)]
pub struct GeneratedTrackInfo {
    pub seed: u64,
    pub piece_count: usize,
    pub checkpoint_count: usize,
    pub road_surface_count: usize,
    pub rail_count: usize,
    pub trigger_count: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackBounds {
    pub center: Vec2,
    pub half_extents: Vec2,
}

impl TrackBounds {
    pub fn from_pieces(pieces: &[TrackPiece]) -> Self {
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);

        for frame in pieces.iter().flat_map(|piece| piece.frames.iter()) {
            min = min.min(frame.pose.position);
            max = max.max(frame.pose.position);
        }

        if !min.is_finite() || !max.is_finite() {
            return Self {
                center: Vec2::ZERO,
                half_extents: Vec2::splat(45.0),
            };
        }

        let center = (min + max) * 0.5;
        let half_extents = ((max - min) * 0.5) + Vec2::splat(TRACK_WIDTH * 2.5);

        Self {
            center,
            half_extents,
        }
    }

    pub fn grass_size(self) -> Vec2 {
        (self.half_extents + Vec2::splat(28.0)) * 2.0
    }
}

impl GeneratedTrackInfo {
    pub fn from_pieces(recipe: &TrackRecipe, pieces: &[TrackPiece]) -> Self {
        Self {
            seed: recipe.seed,
            piece_count: pieces.len(),
            checkpoint_count: TrackPiece::checkpoint_count(pieces),
            road_surface_count: pieces.iter().map(TrackPiece::segment_count).sum(),
            rail_count: pieces.iter().map(TrackPiece::rail_count).sum(),
            trigger_count: TrackPiece::trigger_count(pieces),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TrackPieceKind {
    Straight,
    Curve(TurnDirection),
    Checkpoint(usize),
    Finish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnDirection {
    Left,
    Right,
}

impl TurnDirection {
    fn side(self) -> f32 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrackPiece {
    pub kind: TrackPieceKind,
    pub surface: SurfaceKind,
    pub frames: Vec<PathFrame>,
}

#[derive(Clone, Copy, Debug)]
pub struct PathFrame {
    pub pose: Pose2,
}

impl TrackPiece {
    pub fn entry(&self) -> Pose2 {
        self.frames
            .first()
            .map(|frame| frame.pose)
            .unwrap_or_else(|| Pose2::new(Vec2::ZERO, 0.0))
    }

    pub fn exit(&self) -> Pose2 {
        self.frames
            .last()
            .map(|frame| frame.pose)
            .unwrap_or_else(|| self.entry())
    }

    pub fn checkpoint_count(pieces: &[Self]) -> usize {
        pieces
            .iter()
            .filter(|piece| matches!(piece.kind, TrackPieceKind::Checkpoint(_)))
            .count()
    }
}

pub fn generate_track_pieces(recipe: &TrackRecipe) -> Vec<TrackPiece> {
    let piece_count = recipe.piece_count.max(4);
    let checkpoint_index = piece_count / 2;
    let mut rng = ChaCha8Rng::seed_from_u64(recipe.seed);
    let mut entry = Pose2::new(
        Vec2::new(0.0, -((piece_count as f32) * PIECE_LENGTH * 0.5)),
        0.0,
    );
    let mut pieces = Vec::with_capacity(piece_count);
    let mut occupied_roads = Vec::with_capacity(piece_count);

    for index in 0..piece_count {
        let candidates = piece_kind_candidates(
            index,
            piece_count,
            checkpoint_index,
            recipe.difficulty,
            pieces
                .last()
                .is_some_and(|piece: &TrackPiece| matches!(piece.kind, TrackPieceKind::Curve(_))),
            &mut rng,
        );

        let mut accepted = None;
        for kind in candidates {
            let surface = match kind {
                TrackPieceKind::Finish => SurfaceKind::Boost,
                _ => generated_surface(recipe.surface_mix, &mut rng),
            };
            let frames = generated_frames(entry, kind, recipe.difficulty);
            let piece = TrackPiece {
                kind,
                surface,
                frames,
            };

            if candidate_is_valid(&pieces, &occupied_roads, &piece, index).is_ok() {
                accepted = Some(piece);
                break;
            }
        }

        let piece = accepted.unwrap_or_else(|| {
            let kind = required_piece_kind(index, piece_count, checkpoint_index)
                .unwrap_or(TrackPieceKind::Straight);
            TrackPiece {
                kind,
                surface: generated_surface(recipe.surface_mix, &mut rng),
                frames: generated_frames(entry, kind, recipe.difficulty),
            }
        });

        for road in piece.geometry().roads {
            occupied_roads.push(road.bounds);
        }
        entry = piece.exit();
        pieces.push(piece);
    }

    pieces
}

pub fn validate_track_pieces(pieces: &[TrackPiece]) -> Result<(), String> {
    if pieces.is_empty() {
        return Err("track has no pieces".to_string());
    }

    let finish_count = pieces
        .iter()
        .filter(|piece| matches!(piece.kind, TrackPieceKind::Finish))
        .count();
    if finish_count != 1 {
        return Err(format!(
            "track has {finish_count} finish pieces, expected 1"
        ));
    }

    for (index, piece) in pieces.iter().enumerate() {
        validate_piece_primitive(piece, index)?;
    }

    validate_route_rules(pieces)?;

    for (index, pair) in pieces.windows(2).enumerate() {
        let previous = &pair[0];
        let next = &pair[1];
        validate_piece_connection(index, previous, next)?;
    }

    let info = GeneratedTrackInfo {
        seed: 0,
        piece_count: pieces.len(),
        checkpoint_count: TrackPiece::checkpoint_count(pieces),
        road_surface_count: pieces.iter().map(TrackPiece::segment_count).sum(),
        rail_count: pieces.iter().map(TrackPiece::rail_count).sum(),
        trigger_count: TrackPiece::trigger_count(pieces),
    };

    if info.road_surface_count == 0 || info.rail_count == 0 || info.trigger_count == 0 {
        return Err(format!(
            "invalid generated counts: roads {}, rails {}, triggers {}",
            info.road_surface_count, info.rail_count, info.trigger_count
        ));
    }

    validate_no_road_overlaps(pieces)?;

    Ok(())
}

pub fn car_spawn_for(pieces: &[TrackPiece]) -> CarSpawn {
    let Some(first_piece) = pieces.first() else {
        return CarSpawn::default();
    };
    let entry = first_piece.entry();
    let start = entry.position + forward_2d(entry.yaw) * 1.1;

    CarSpawn {
        translation: Vec3::new(start.x, 0.05, start.y),
        yaw: entry.yaw,
    }
}

pub fn forward_2d(yaw: f32) -> Vec2 {
    Pose2::new(Vec2::ZERO, yaw).forward()
}

fn piece_kind_candidates(
    index: usize,
    piece_count: usize,
    checkpoint_index: usize,
    difficulty: u8,
    previous_was_curve: bool,
    rng: &mut ChaCha8Rng,
) -> Vec<TrackPieceKind> {
    if let Some(required) = required_piece_kind(index, piece_count, checkpoint_index) {
        return vec![required];
    }

    if should_place_curve(difficulty, previous_was_curve, rng) {
        let primary = random_turn_direction(rng);
        let secondary = match primary {
            TurnDirection::Left => TurnDirection::Right,
            TurnDirection::Right => TurnDirection::Left,
        };
        vec![
            TrackPieceKind::Curve(primary),
            TrackPieceKind::Straight,
            TrackPieceKind::Curve(secondary),
        ]
    } else {
        vec![TrackPieceKind::Straight]
    }
}

fn required_piece_kind(
    index: usize,
    piece_count: usize,
    checkpoint_index: usize,
) -> Option<TrackPieceKind> {
    if index == piece_count - 1 {
        Some(TrackPieceKind::Finish)
    } else if index == checkpoint_index {
        Some(TrackPieceKind::Checkpoint(0))
    } else {
        None
    }
}

fn should_place_curve(difficulty: u8, previous_was_curve: bool, rng: &mut ChaCha8Rng) -> bool {
    if previous_was_curve {
        return false;
    }

    let straight_weight = match difficulty {
        0 => 8,
        1 => 6,
        2 => 4,
        _ => 3,
    };

    rng.random_range(0..10) >= straight_weight
}

fn random_turn_direction(rng: &mut ChaCha8Rng) -> TurnDirection {
    if rng.random_bool(0.5) {
        TurnDirection::Right
    } else {
        TurnDirection::Left
    }
}

fn generated_surface(surface_mix: SurfaceMix, rng: &mut ChaCha8Rng) -> SurfaceKind {
    match (surface_mix, rng.random_range(0..12)) {
        (SurfaceMix::Balanced, 0..=6) => SurfaceKind::Asphalt,
        (SurfaceMix::Balanced, 7..=8) => SurfaceKind::Dirt,
        (SurfaceMix::Balanced, 9..=10) => SurfaceKind::Ice,
        (SurfaceMix::Balanced, _) => SurfaceKind::Boost,
        (SurfaceMix::Technical, 0..=3) => SurfaceKind::Asphalt,
        (SurfaceMix::Technical, 4..=7) => SurfaceKind::Dirt,
        (SurfaceMix::Technical, 8..=10) => SurfaceKind::Ice,
        (SurfaceMix::Technical, _) => SurfaceKind::Boost,
        (SurfaceMix::Fast, 0..=7) => SurfaceKind::Asphalt,
        (SurfaceMix::Fast, 8) => SurfaceKind::Dirt,
        (SurfaceMix::Fast, 9) => SurfaceKind::Ice,
        (SurfaceMix::Fast, _) => SurfaceKind::Boost,
    }
}

fn generated_frames(entry: Pose2, kind: TrackPieceKind, difficulty: u8) -> Vec<PathFrame> {
    match kind {
        TrackPieceKind::Straight | TrackPieceKind::Checkpoint(_) | TrackPieceKind::Finish => {
            straight_frames(entry)
        }
        TrackPieceKind::Curve(direction) => curve_frames(entry, direction, difficulty),
    }
}

fn straight_frames(entry: Pose2) -> Vec<PathFrame> {
    let exit = Pose2::new(entry.position + entry.forward() * PIECE_LENGTH, entry.yaw);
    vec![PathFrame { pose: entry }, PathFrame { pose: exit }]
}

fn curve_frames(entry: Pose2, direction: TurnDirection, difficulty: u8) -> Vec<PathFrame> {
    const CURVE_RADIUS: f32 = 24.0;
    const CURVE_STEPS: usize = 6;

    let curve_angle = match difficulty {
        0 => std::f32::consts::FRAC_PI_6,
        1 => std::f32::consts::FRAC_PI_4,
        _ => std::f32::consts::FRAC_PI_3,
    };
    let side = direction.side();
    let center = entry.position + entry.right() * side * CURVE_RADIUS;
    let radius_vector = -entry.right() * side * CURVE_RADIUS;

    (0..=CURVE_STEPS)
        .map(|step| {
            let t = step as f32 / CURVE_STEPS as f32;
            let angle = side * curve_angle * t;
            PathFrame {
                pose: Pose2::new(
                    center + rotate_2d(radius_vector, angle),
                    entry.yaw + side * curve_angle * t,
                ),
            }
        })
        .collect()
}

fn candidate_is_valid(
    pieces: &[TrackPiece],
    occupied_roads: &[OrientedRect],
    candidate: &TrackPiece,
    index: usize,
) -> Result<(), String> {
    validate_piece_primitive(candidate, index)?;

    if let Some(previous) = pieces.last() {
        validate_piece_connection(index.saturating_sub(1), previous, candidate)?;
    }

    if candidate.exit().yaw.abs() > MAX_ROUTE_YAW {
        return Err(format!(
            "piece {index} exits with route yaw {:.3}, max {:.3}",
            candidate.exit().yaw,
            MAX_ROUTE_YAW
        ));
    }

    let candidate_roads: Vec<_> = candidate
        .geometry()
        .roads
        .into_iter()
        .map(|road| road.bounds)
        .collect();

    validate_candidate_internal_overlaps(index, &candidate_roads)?;
    validate_candidate_against_occupied(index, occupied_roads, &candidate_roads)?;

    Ok(())
}

fn validate_piece_primitive(piece: &TrackPiece, index: usize) -> Result<(), String> {
    if piece.frames.len() < 2 {
        return Err(format!(
            "piece {index} has {} frames, expected at least 2",
            piece.frames.len()
        ));
    }

    let geometry = piece.geometry();
    for (segment_index, road) in geometry.roads.iter().enumerate() {
        if road.length <= 0.001 {
            return Err(format!(
                "piece {index} segment {segment_index} has nonpositive length {:.4}",
                road.length
            ));
        }
    }

    if geometry.rails.len() != geometry.roads.len() * 2 {
        return Err(format!(
            "piece {index} has {} rail spans for {} road spans",
            geometry.rails.len(),
            geometry.roads.len()
        ));
    }

    if let Some(trigger) = geometry.trigger {
        let expected_pose = match piece.kind {
            TrackPieceKind::Straight | TrackPieceKind::Curve(_) => piece.entry(),
            TrackPieceKind::Checkpoint(_) => piece.entry(),
            TrackPieceKind::Finish => piece.exit(),
        };
        let offset = trigger
            .bounds
            .pose
            .position
            .distance(expected_pose.position);
        let yaw_delta = (trigger.bounds.pose.yaw - expected_pose.yaw).abs();

        if offset > 0.001 || yaw_delta > 0.001 {
            return Err(format!(
                "piece {index} trigger is misaligned by {:.4} and yaw {:.4}",
                offset, yaw_delta
            ));
        }
    }

    match piece.kind {
        TrackPieceKind::Straight | TrackPieceKind::Checkpoint(_) | TrackPieceKind::Finish => {
            if piece.frames.len() != 2 {
                return Err(format!(
                    "piece {index} has {} frames for a straight-aligned piece, expected 2",
                    piece.frames.len()
                ));
            }
        }
        TrackPieceKind::Curve(_) => {
            if piece.frames.len() < 3 {
                return Err(format!(
                    "piece {index} has {} curve frames, expected at least 3",
                    piece.frames.len()
                ));
            }
            validate_curve_frames(piece, index)?;
        }
    }

    Ok(())
}

fn validate_curve_frames(piece: &TrackPiece, index: usize) -> Result<(), String> {
    let first_segment = piece.frames[0]
        .pose
        .position
        .distance(piece.frames[1].pose.position);
    let mut previous_yaw = piece.frames[0].pose.yaw;
    let yaw_direction = (piece.exit().yaw - piece.entry().yaw).signum();

    if yaw_direction == 0.0 {
        return Err(format!("piece {index} curve has no yaw change"));
    }

    for (frame_index, pair) in piece.frames.windows(2).enumerate() {
        let segment_length = pair[0].pose.position.distance(pair[1].pose.position);
        if segment_length <= 0.001 {
            return Err(format!(
                "piece {index} curve segment {frame_index} has nonpositive length {:.4}",
                segment_length
            ));
        }

        let length_ratio = segment_length / first_segment.max(0.001);
        if !(0.75..=1.25).contains(&length_ratio) {
            return Err(format!(
                "piece {index} curve segment {frame_index} length ratio {:.3} is outside coherent range",
                length_ratio
            ));
        }

        let yaw_delta = pair[1].pose.yaw - previous_yaw;
        if yaw_delta.signum() != yaw_direction {
            return Err(format!(
                "piece {index} curve frame {} reverses yaw progression",
                frame_index + 1
            ));
        }
        previous_yaw = pair[1].pose.yaw;
    }

    Ok(())
}

fn validate_piece_connection(
    previous_index: usize,
    previous: &TrackPiece,
    next: &TrackPiece,
) -> Result<(), String> {
    let gap = previous.exit().position.distance(next.entry().position);
    let yaw_delta = (previous.exit().yaw - next.entry().yaw).abs();

    if gap > 0.001 || yaw_delta > 0.001 {
        return Err(format!(
            "piece {} -> {} has gap {:.4} and yaw delta {:.4}",
            previous_index,
            previous_index + 1,
            gap,
            yaw_delta
        ));
    }

    Ok(())
}

fn validate_candidate_internal_overlaps(
    piece_index: usize,
    candidate_roads: &[OrientedRect],
) -> Result<(), String> {
    for first_index in 0..candidate_roads.len() {
        for second_index in (first_index + 2)..candidate_roads.len() {
            if road_rects_overlap(candidate_roads[first_index], candidate_roads[second_index]) {
                return Err(format!(
                    "piece {piece_index} has overlapping road segments {first_index} and {second_index}"
                ));
            }
        }
    }

    Ok(())
}

fn validate_candidate_against_occupied(
    piece_index: usize,
    occupied_roads: &[OrientedRect],
    candidate_roads: &[OrientedRect],
) -> Result<(), String> {
    let allowed_connection_index = occupied_roads.len().checked_sub(1);

    for (candidate_index, candidate) in candidate_roads.iter().copied().enumerate() {
        for (occupied_index, occupied) in occupied_roads.iter().copied().enumerate() {
            if Some(occupied_index) == allowed_connection_index && candidate_index == 0 {
                continue;
            }

            if road_rects_overlap(candidate, occupied) {
                return Err(format!(
                    "piece {piece_index} road segment {candidate_index} overlaps existing segment {occupied_index}"
                ));
            }
        }
    }

    Ok(())
}

fn validate_no_road_overlaps(pieces: &[TrackPiece]) -> Result<(), String> {
    let road_bounds: Vec<_> = pieces
        .iter()
        .flat_map(|piece| piece.geometry().roads.into_iter().map(|road| road.bounds))
        .collect();

    for first_index in 0..road_bounds.len() {
        for second_index in (first_index + 2)..road_bounds.len() {
            if road_rects_overlap(road_bounds[first_index], road_bounds[second_index]) {
                return Err(format!(
                    "road segment {first_index} overlaps non-adjacent road segment {second_index}"
                ));
            }
        }
    }

    Ok(())
}

fn validate_route_rules(pieces: &[TrackPiece]) -> Result<(), String> {
    let checkpoint_index = pieces
        .iter()
        .position(|piece| matches!(piece.kind, TrackPieceKind::Checkpoint(_)));
    let finish_index = pieces
        .iter()
        .position(|piece| matches!(piece.kind, TrackPieceKind::Finish));

    match (checkpoint_index, finish_index) {
        (Some(checkpoint), Some(finish)) if checkpoint < finish => {}
        (Some(checkpoint), Some(finish)) => {
            return Err(format!(
                "checkpoint at piece {checkpoint} must come before finish at piece {finish}"
            ));
        }
        (None, _) => return Err("track has no checkpoint".to_string()),
        (_, None) => return Err("track has no finish".to_string()),
    }

    for (index, piece) in pieces.iter().enumerate() {
        let yaw = piece.exit().yaw.abs();
        if yaw > MAX_ROUTE_YAW {
            return Err(format!(
                "piece {index} exits with route yaw {:.3}, max {:.3}",
                piece.exit().yaw,
                MAX_ROUTE_YAW
            ));
        }
    }

    for (index, pair) in pieces.windows(2).enumerate() {
        if matches!(pair[0].kind, TrackPieceKind::Curve(_))
            && matches!(pair[1].kind, TrackPieceKind::Curve(_))
        {
            return Err(format!(
                "piece {index} and {} are adjacent curves without recovery",
                index + 1
            ));
        }
    }

    Ok(())
}

fn road_rects_overlap(a: OrientedRect, b: OrientedRect) -> bool {
    let Some(a) = a.shrunken(ROAD_OVERLAP_SHRINK) else {
        return false;
    };
    let Some(b) = b.shrunken(ROAD_OVERLAP_SHRINK) else {
        return false;
    };

    a.intersects(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tracks_validate_across_seed_range() {
        for seed in 0..128 {
            let recipe = TrackRecipe {
                seed,
                piece_count: 32,
                difficulty: 3,
                surface_mix: SurfaceMix::Technical,
            };
            let pieces = generate_track_pieces(&recipe);

            validate_track_pieces(&pieces).unwrap_or_else(|error| {
                panic!("seed {seed} generated invalid track: {error}");
            });
        }
    }

    #[test]
    fn candidate_validation_rejects_non_adjacent_overlap() {
        let first = TrackPiece {
            kind: TrackPieceKind::Straight,
            surface: SurfaceKind::Asphalt,
            frames: straight_frames(Pose2::new(Vec2::ZERO, 0.0)),
        };
        let occupied = first
            .geometry()
            .roads
            .into_iter()
            .map(|road| road.bounds)
            .collect::<Vec<_>>();
        let overlapping = TrackPiece {
            kind: TrackPieceKind::Straight,
            surface: SurfaceKind::Asphalt,
            frames: straight_frames(Pose2::new(Vec2::new(0.0, 2.0), 0.0)),
        };

        let error = candidate_is_valid(&[first], &occupied, &overlapping, 1)
            .expect_err("overlapping candidate should be rejected");

        assert!(error.contains("gap") || error.contains("overlaps"));
    }

    #[test]
    fn route_validation_rejects_adjacent_curves() {
        let first = TrackPiece {
            kind: TrackPieceKind::Curve(TurnDirection::Right),
            surface: SurfaceKind::Asphalt,
            frames: curve_frames(Pose2::new(Vec2::ZERO, 0.0), TurnDirection::Right, 1),
        };
        let second = TrackPiece {
            kind: TrackPieceKind::Curve(TurnDirection::Left),
            surface: SurfaceKind::Asphalt,
            frames: curve_frames(first.exit(), TurnDirection::Left, 1),
        };
        let checkpoint = TrackPiece {
            kind: TrackPieceKind::Checkpoint(0),
            surface: SurfaceKind::Asphalt,
            frames: straight_frames(second.exit()),
        };
        let finish = TrackPiece {
            kind: TrackPieceKind::Finish,
            surface: SurfaceKind::Boost,
            frames: straight_frames(checkpoint.exit()),
        };

        let error = validate_track_pieces(&[first, second, checkpoint, finish])
            .expect_err("adjacent curves should be rejected");

        assert!(error.contains("adjacent curves"));
    }
}
