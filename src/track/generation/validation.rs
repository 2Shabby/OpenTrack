use super::types::{GeneratedTrackInfo, RAIL_THICKNESS, TRACK_WIDTH, TrackPiece, TrackPieceKind};
use std::collections::HashSet;

const MAX_ROUTE_YAW: f32 = std::f32::consts::PI * 0.85;
const OCCUPANCY_CELL_SIZE: f32 = 6.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct OccupancyCell {
    x: i32,
    y: i32,
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
        road_surface_count: pieces.len(),
        rail_count: pieces.iter().map(TrackPiece::rail_count).sum(),
        trigger_count: TrackPiece::trigger_count(pieces),
    };

    if info.road_surface_count == 0 || info.rail_count == 0 || info.trigger_count == 0 {
        return Err(format!(
            "invalid generated counts: roads {}, rails {}, triggers {}",
            info.road_surface_count, info.rail_count, info.trigger_count
        ));
    }

    validate_no_occupied_cell_reuse(pieces)?;

    Ok(())
}

pub(crate) fn candidate_is_valid(
    occupied: &HashSet<OccupancyCell>,
    candidate: &TrackPiece,
    index: usize,
) -> Result<(), String> {
    validate_piece_primitive(candidate, index)?;

    if candidate.exit().yaw.abs() > MAX_ROUTE_YAW {
        return Err(format!(
            "piece {index} exits with route yaw {:.3}, max {:.3}",
            candidate.exit().yaw,
            MAX_ROUTE_YAW
        ));
    }

    for cell in occupied_cells_for_fit(candidate) {
        if occupied.contains(&cell) {
            return Err(format!("piece {index} enters occupied sector {cell:?}"));
        }
    }

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
            TrackPieceKind::Straight | TrackPieceKind::Turn { .. } => piece.entry(),
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
        TrackPieceKind::Turn { .. } => {
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

fn validate_no_occupied_cell_reuse(pieces: &[TrackPiece]) -> Result<(), String> {
    let mut occupied = HashSet::new();

    for (index, piece) in pieces.iter().enumerate() {
        for cell in occupied_cells_for_fit(piece) {
            if !occupied.insert(cell) {
                return Err(format!("piece {index} reuses occupied sector {cell:?}"));
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
    Ok(())
}

pub(crate) fn occupied_cells(piece: &TrackPiece) -> Vec<OccupancyCell> {
    let mut cells = Vec::new();
    for road in piece.geometry().roads {
        for cell in OccupancyCell::from_bounds(road.bounds.corners()) {
            if road.bounds.contains(cell.center()) && !cells.contains(&cell) {
                cells.push(cell);
            }
        }
    }
    cells
}

pub(crate) fn occupied_cells_for_fit(
    piece: &TrackPiece,
) -> impl Iterator<Item = OccupancyCell> + '_ {
    let seam_cells = OccupancyCell::from_entry_seam(piece.entry());
    occupied_cells(piece)
        .into_iter()
        .filter(move |cell| !seam_cells.contains(cell))
}

impl OccupancyCell {
    fn from_bounds(corners: [bevy::prelude::Vec2; 4]) -> Vec<Self> {
        let mut min = bevy::prelude::Vec2::splat(f32::INFINITY);
        let mut max = bevy::prelude::Vec2::splat(f32::NEG_INFINITY);

        for corner in corners {
            min = min.min(corner);
            max = max.max(corner);
        }

        let min_cell = Self::from_position(min);
        let max_cell = Self::from_position(max);
        let mut cells = Vec::new();

        for x in min_cell.x..=max_cell.x {
            for y in min_cell.y..=max_cell.y {
                cells.push(Self { x, y });
            }
        }

        cells
    }

    fn from_entry_seam(entry: crate::geometry::Pose2) -> HashSet<Self> {
        let seam_half_width = TRACK_WIDTH * 0.5 + RAIL_THICKNESS;
        let seam_half_depth = OCCUPANCY_CELL_SIZE * 0.5;
        let forward = entry.forward();
        let right = entry.right();
        let corners = [
            entry.position - right * seam_half_width - forward * seam_half_depth,
            entry.position + right * seam_half_width - forward * seam_half_depth,
            entry.position + right * seam_half_width + forward * seam_half_depth,
            entry.position - right * seam_half_width + forward * seam_half_depth,
        ];

        Self::from_bounds(corners).into_iter().collect()
    }

    fn from_position(position: bevy::prelude::Vec2) -> Self {
        Self {
            x: (position.x / OCCUPANCY_CELL_SIZE).floor() as i32,
            y: (position.y / OCCUPANCY_CELL_SIZE).floor() as i32,
        }
    }

    fn center(self) -> bevy::prelude::Vec2 {
        bevy::prelude::Vec2::new(
            (self.x as f32 + 0.5) * OCCUPANCY_CELL_SIZE,
            (self.y as f32 + 0.5) * OCCUPANCY_CELL_SIZE,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Pose2;
    use crate::surface::SurfaceKind;

    use super::super::assembly::generate_track_pieces;
    use super::super::path::TrackPath;
    use super::super::types::{TrackRecipe, TurnAngle, TurnDirection};

    #[test]
    fn generated_tracks_validate_across_seed_range() {
        for seed in 0..128 {
            let recipe = TrackRecipe {
                seed,
                piece_count: 32,
            };
            let pieces = generate_track_pieces(&recipe);

            validate_track_pieces(&pieces).unwrap_or_else(|error| {
                panic!("seed {seed} generated invalid track: {error}");
            });
        }
    }

    #[test]
    fn generated_tracks_include_curves_across_seed_range() {
        for seed in 0..128 {
            let recipe = TrackRecipe {
                seed,
                piece_count: 16,
            };
            let pieces = generate_track_pieces(&recipe);

            assert!(
                pieces
                    .iter()
                    .any(|piece| matches!(piece.kind, TrackPieceKind::Turn { .. })),
                "seed {seed} generated no turn pieces"
            );
        }
    }

    #[test]
    fn candidate_validation_rejects_occupied_sector_entry() {
        let first = TrackPiece {
            kind: TrackPieceKind::Straight,
            surface: SurfaceKind::Asphalt,
            frames: test_frames(
                Pose2::new(bevy::prelude::Vec2::ZERO, 0.0),
                TrackPieceKind::Straight,
            ),
        };
        let occupied = occupied_cells(&first).into_iter().collect::<HashSet<_>>();
        let overlapping = TrackPiece {
            kind: TrackPieceKind::Straight,
            surface: SurfaceKind::Asphalt,
            frames: test_frames(
                Pose2::new(bevy::prelude::Vec2::new(0.0, 2.0), 0.0),
                TrackPieceKind::Straight,
            ),
        };

        let error = candidate_is_valid(&occupied, &overlapping, 1)
            .expect_err("occupied-sector candidate should be rejected");

        assert!(error.contains("occupied sector"));
    }

    #[test]
    fn route_validation_allows_adjacent_turns_when_connected_and_non_overlapping() {
        let first = TrackPiece {
            kind: TrackPieceKind::Turn {
                direction: TurnDirection::Right,
                angle: TurnAngle::Deg45,
            },
            surface: SurfaceKind::Asphalt,
            frames: test_frames(
                Pose2::new(bevy::prelude::Vec2::ZERO, 0.0),
                TrackPieceKind::Turn {
                    direction: TurnDirection::Right,
                    angle: TurnAngle::Deg45,
                },
            ),
        };
        let second = TrackPiece {
            kind: TrackPieceKind::Turn {
                direction: TurnDirection::Right,
                angle: TurnAngle::Deg45,
            },
            surface: SurfaceKind::Asphalt,
            frames: test_frames(
                first.exit(),
                TrackPieceKind::Turn {
                    direction: TurnDirection::Right,
                    angle: TurnAngle::Deg45,
                },
            ),
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

        validate_track_pieces(&[first, second, checkpoint, finish]).unwrap();
    }

    fn test_frames(entry: Pose2, kind: TrackPieceKind) -> Vec<super::super::types::PathFrame> {
        TrackPath::for_piece(entry, kind).sample_frames()
    }
}
