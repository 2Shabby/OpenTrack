use super::types::{GeneratedTrackInfo, TrackPiece, TrackPieceKind};
use crate::geometry::OrientedRect;

const MAX_ROUTE_YAW: f32 = std::f32::consts::PI * 0.85;
const ROAD_OVERLAP_SHRINK: f32 = 0.35;

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

pub(crate) fn candidate_is_valid(
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
    use crate::geometry::Pose2;
    use crate::surface::SurfaceKind;

    use super::super::assembly::generate_track_pieces;
    use super::super::path::TrackPath;
    use super::super::types::{SurfaceMix, TrackRecipe, TurnDirection};

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
            frames: test_frames(
                Pose2::new(bevy::prelude::Vec2::ZERO, 0.0),
                TrackPieceKind::Straight,
            ),
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
            frames: test_frames(
                Pose2::new(bevy::prelude::Vec2::new(0.0, 2.0), 0.0),
                TrackPieceKind::Straight,
            ),
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
            frames: test_frames(
                Pose2::new(bevy::prelude::Vec2::ZERO, 0.0),
                TrackPieceKind::Curve(TurnDirection::Right),
            ),
        };
        let second = TrackPiece {
            kind: TrackPieceKind::Curve(TurnDirection::Left),
            surface: SurfaceKind::Asphalt,
            frames: test_frames(first.exit(), TrackPieceKind::Curve(TurnDirection::Left)),
        };
        let checkpoint = TrackPiece {
            kind: TrackPieceKind::Checkpoint(0),
            surface: SurfaceKind::Asphalt,
            frames: test_frames(second.exit(), TrackPieceKind::Checkpoint(0)),
        };
        let finish = TrackPiece {
            kind: TrackPieceKind::Finish,
            surface: SurfaceKind::Boost,
            frames: test_frames(checkpoint.exit(), TrackPieceKind::Finish),
        };

        let error = validate_track_pieces(&[first, second, checkpoint, finish])
            .expect_err("adjacent curves should be rejected");

        assert!(error.contains("adjacent curves"));
    }

    fn test_frames(entry: Pose2, kind: TrackPieceKind) -> Vec<super::super::types::PathFrame> {
        TrackPath::for_piece(entry, kind, 1).sample_frames()
    }
}
