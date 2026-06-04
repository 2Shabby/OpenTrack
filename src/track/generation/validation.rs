use super::types::{
    BankTransitionMode, GeneratedTrackInfo, MAX_BANK_ANGLE, RAIL_THICKNESS, TRACK_WIDTH,
    TrackConnector, TrackPiece, TrackPieceKind,
};
use std::collections::HashSet;

const MAX_ROUTE_YAW: f32 = std::f32::consts::PI * 0.85;
const OCCUPANCY_CELL_SIZE: f32 = 7.2;
const MAX_BANK_DELTA_PER_METER: f32 = 0.07;

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
        rail_count: TrackPiece::rail_count(pieces),
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

    if geometry.rails.len() != 2 {
        return Err(format!(
            "piece {index} has {} rail paths, expected 2",
            geometry.rails.len(),
        ));
    }
    for (rail_index, rail) in geometry.rails.iter().enumerate() {
        if rail.points.len() != piece.frames.len() {
            return Err(format!(
                "piece {index} rail {rail_index} has {} points for {} frames",
                rail.points.len(),
                piece.frames.len()
            ));
        }
        for (segment_index, pair) in rail.points.windows(2).enumerate() {
            if pair[0].distance(pair[1]) <= 0.001 {
                return Err(format!(
                    "piece {index} rail {rail_index} segment {segment_index} has nonpositive length"
                ));
            }
        }
    }

    if let Some(trigger) = geometry.trigger {
        let expected_pose = match piece.kind {
            TrackPieceKind::Straight
            | TrackPieceKind::DoubleStraight
            | TrackPieceKind::BankTransition { .. }
            | TrackPieceKind::BankedStraight { .. }
            | TrackPieceKind::BankedDoubleStraight { .. }
            | TrackPieceKind::Turn { .. } => piece.entry(),
            TrackPieceKind::BankedTurn { .. } => piece.entry(),
            TrackPieceKind::Checkpoint(_) => piece.entry(),
            TrackPieceKind::Finish => piece.exit(),
        };
        let offset = trigger
            .bounds
            .pose
            .position
            .distance(expected_pose.position);
        let yaw_delta = (trigger.bounds.pose.yaw - expected_pose.yaw).abs();
        let bank_delta = (trigger.frame.bank - expected_pose.bank).abs();

        if offset > 0.001 || yaw_delta > 0.001 || bank_delta > 0.001 {
            return Err(format!(
                "piece {index} trigger is misaligned by {:.4}, yaw {:.4}, and bank {:.4}",
                offset, yaw_delta, bank_delta
            ));
        }
    }

    match piece.kind {
        TrackPieceKind::Straight
        | TrackPieceKind::DoubleStraight
        | TrackPieceKind::BankedStraight { .. }
        | TrackPieceKind::BankedDoubleStraight { .. }
        | TrackPieceKind::Checkpoint(_)
        | TrackPieceKind::Finish => {
            if piece.frames.len() != 2 {
                return Err(format!(
                    "piece {index} has {} frames for a straight-aligned piece, expected 2",
                    piece.frames.len()
                ));
            }
        }
        TrackPieceKind::BankTransition { .. } => {
            if piece.frames.len() < 4 {
                return Err(format!(
                    "piece {index} has {} frames for a bank transition, expected at least 4",
                    piece.frames.len()
                ));
            }
            validate_straight_frames(piece, index)?;
        }
        TrackPieceKind::Turn { .. } | TrackPieceKind::BankedTurn { .. } => {
            if piece.frames.len() < 3 {
                return Err(format!(
                    "piece {index} has {} curve frames, expected at least 3",
                    piece.frames.len()
                ));
            }
            validate_curve_frames(piece, index)?;
        }
    }
    validate_bank_frames(piece, index)?;
    validate_bank_profile(piece, index)?;

    Ok(())
}

fn validate_straight_frames(piece: &TrackPiece, index: usize) -> Result<(), String> {
    let first_segment = piece.frames[0].position.distance(piece.frames[1].position);
    let expected_yaw = piece.entry().yaw;
    let expected_forward = piece.entry().forward();

    for (frame_index, pair) in piece.frames.windows(2).enumerate() {
        for frame in pair {
            let yaw_delta = (frame.yaw - expected_yaw).abs();
            if yaw_delta > 0.001 {
                return Err(format!(
                    "piece {index} straight frame {frame_index} changes yaw by {:.4}",
                    yaw_delta
                ));
            }
        }

        let segment = pair[1].position - pair[0].position;
        let segment_length = segment.length();
        if segment_length <= 0.001 {
            return Err(format!(
                "piece {index} straight segment {frame_index} has nonpositive length {:.4}",
                segment_length
            ));
        }

        let length_ratio = segment_length / first_segment.max(0.001);
        if !(0.75..=1.25).contains(&length_ratio) {
            return Err(format!(
                "piece {index} straight segment {frame_index} length ratio {:.3} is outside coherent range",
                length_ratio
            ));
        }

        let direction = segment / segment_length;
        if direction.dot(expected_forward) < 0.999 {
            return Err(format!(
                "piece {index} straight segment {frame_index} does not follow entry forward"
            ));
        }
    }

    Ok(())
}

fn validate_curve_frames(piece: &TrackPiece, index: usize) -> Result<(), String> {
    let first_segment = piece.frames[0].position.distance(piece.frames[1].position);
    let mut previous_yaw = piece.frames[0].yaw;
    let yaw_direction = (piece.exit().yaw - piece.entry().yaw).signum();

    if yaw_direction == 0.0 {
        return Err(format!("piece {index} curve has no yaw change"));
    }

    for (frame_index, pair) in piece.frames.windows(2).enumerate() {
        let segment_length = pair[0].position.distance(pair[1].position);
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

        let yaw_delta = pair[1].yaw - previous_yaw;
        if yaw_delta.signum() != yaw_direction {
            return Err(format!(
                "piece {index} curve frame {} reverses yaw progression",
                frame_index + 1
            ));
        }
        previous_yaw = pair[1].yaw;
    }

    Ok(())
}

fn validate_bank_frames(piece: &TrackPiece, index: usize) -> Result<(), String> {
    for (frame_index, frame) in piece.frames.iter().enumerate() {
        if frame.bank.abs() > MAX_BANK_ANGLE + 0.001 {
            return Err(format!(
                "piece {index} frame {frame_index} has bank {:.3}, max {:.3}",
                frame.bank, MAX_BANK_ANGLE
            ));
        }
    }

    for (frame_index, pair) in piece.frames.windows(2).enumerate() {
        let distance = pair[0].center.distance(pair[1].center).max(0.001);
        let bank_rate = (pair[1].bank - pair[0].bank).abs() / distance;
        if bank_rate > MAX_BANK_DELTA_PER_METER {
            return Err(format!(
                "piece {index} frame {frame_index} changes bank too quickly: {:.3}",
                bank_rate
            ));
        }
    }

    Ok(())
}

fn validate_bank_profile(piece: &TrackPiece, index: usize) -> Result<(), String> {
    let entry_bank = piece.entry().bank;
    let exit_bank = piece.exit().bank;

    match piece.kind {
        TrackPieceKind::BankTransition { mode, .. } => match mode {
            BankTransitionMode::In => {
                if entry_bank.abs() > 0.001 || exit_bank.abs() <= 0.001 {
                    return Err(format!("piece {index} has invalid bank-in profile"));
                }
            }
            BankTransitionMode::Out => {
                if entry_bank.abs() <= 0.001 || exit_bank.abs() > 0.001 {
                    return Err(format!("piece {index} has invalid bank-out profile"));
                }
            }
        },
        TrackPieceKind::BankedStraight { direction, angle }
        | TrackPieceKind::BankedDoubleStraight { direction, angle } => {
            let expected = direction.side() * angle.radians();
            if (entry_bank - expected).abs() > 0.001 || (exit_bank - expected).abs() > 0.001 {
                return Err(format!("piece {index} has mismatched held bank"));
            }
        }
        TrackPieceKind::BankedTurn {
            direction,
            bank_angle,
            ..
        } => {
            let expected = direction.side() * bank_angle.radians();
            if (entry_bank - expected).abs() > 0.001 || (exit_bank - expected).abs() > 0.001 {
                return Err(format!("piece {index} has mismatched banked turn"));
            }
        }
        TrackPieceKind::Straight
        | TrackPieceKind::DoubleStraight
        | TrackPieceKind::Turn { .. }
        | TrackPieceKind::Checkpoint(_)
        | TrackPieceKind::Finish => {
            if (entry_bank - exit_bank).abs() > 0.001 {
                return Err(format!("piece {index} changes bank without transition"));
            }
        }
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
    let bank_delta = (previous.exit().bank - next.entry().bank).abs();

    if gap > 0.001 || yaw_delta > 0.001 || bank_delta > 0.001 {
        return Err(format!(
            "piece {} -> {} has gap {:.4}, yaw delta {:.4}, and bank delta {:.4}",
            previous_index,
            previous_index + 1,
            gap,
            yaw_delta,
            bank_delta
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

    fn from_entry_seam(entry: TrackConnector) -> HashSet<Self> {
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
    use super::super::types::{
        BankAngle, BankTransitionMode, PIECE_LENGTH, TrackConnector, TrackRecipe, TurnAngle,
        TurnDirection,
    };

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
                pieces.iter().any(|piece| matches!(
                    piece.kind,
                    TrackPieceKind::Turn { .. } | TrackPieceKind::BankedTurn { .. }
                )),
                "seed {seed} generated no turn pieces"
            );
        }
    }

    #[test]
    fn generated_tracks_include_double_straights_across_seed_range() {
        let mut double_straight_count = 0;

        for seed in 0..128 {
            let recipe = TrackRecipe {
                seed,
                piece_count: 24,
            };
            let pieces = generate_track_pieces(&recipe);

            double_straight_count += pieces
                .iter()
                .filter(|piece| matches!(piece.kind, TrackPieceKind::DoubleStraight))
                .count();
        }

        assert!(double_straight_count > 0);
    }

    #[test]
    fn generated_tracks_include_banked_pieces_across_seed_range() {
        let mut banked_piece_count = 0;

        for seed in 0..128 {
            let recipe = TrackRecipe {
                seed,
                piece_count: 32,
            };
            let pieces = generate_track_pieces(&recipe);

            banked_piece_count += pieces
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
        }

        assert!(banked_piece_count > 0);
    }

    #[test]
    fn double_straight_uses_two_piece_lengths() {
        let frames = test_frames(
            Pose2::new(bevy::prelude::Vec2::ZERO, 0.0),
            TrackPieceKind::DoubleStraight,
        );

        assert_eq!(frames.len(), 2);
        let length = frames[0].position.distance(frames[1].position);
        assert!((length - PIECE_LENGTH * 2.0).abs() <= 0.001);
    }

    #[test]
    fn bank_transition_reaches_requested_bank_without_exceeding_limit() {
        let frames = test_frames(
            Pose2::new(bevy::prelude::Vec2::ZERO, 0.0),
            TrackPieceKind::BankTransition {
                direction: TurnDirection::Right,
                angle: BankAngle::Deg45,
                mode: BankTransitionMode::In,
            },
        );

        assert_eq!(frames[0].bank, 0.0);
        assert!(frames.len() > 2);
        assert!((frames.last().unwrap().bank - BankAngle::Deg45.radians()).abs() <= 0.001);
        assert!(
            frames
                .iter()
                .all(|frame| frame.bank.abs() <= MAX_BANK_ANGLE)
        );
    }

    #[test]
    fn bank_transition_uses_dense_eased_samples() {
        let frames = test_frames(
            Pose2::new(bevy::prelude::Vec2::ZERO, 0.0),
            TrackPieceKind::BankTransition {
                direction: TurnDirection::Right,
                angle: BankAngle::Deg45,
                mode: BankTransitionMode::In,
            },
        );
        let bank_deltas: Vec<_> = frames
            .windows(2)
            .map(|pair| (pair[1].bank - pair[0].bank).abs())
            .collect();
        let middle_delta = bank_deltas[bank_deltas.len() / 2];

        assert!(frames.len() >= 9);
        assert!(bank_deltas[0] < middle_delta);
        assert!(*bank_deltas.last().unwrap() < middle_delta);
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

    fn test_frames(
        entry: impl Into<TrackConnector>,
        kind: TrackPieceKind,
    ) -> Vec<super::super::types::PathFrame> {
        TrackPath::for_piece(entry.into(), kind).sample_frames()
    }
}
