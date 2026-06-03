use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashSet;

use super::path::generated_frames;
use super::types::{PIECE_LENGTH, TrackPiece, TrackPieceKind, TrackRecipe, TurnDirection};
use super::validation::{candidate_is_valid, occupied_cells};
use crate::geometry::Pose2;
use crate::surface::SurfaceKind;

const MAX_PLAN_ATTEMPTS: usize = 256;

pub fn generate_track_pieces(recipe: &TrackRecipe) -> Vec<TrackPiece> {
    let piece_count = recipe.piece_count.max(4);
    let checkpoint_index = piece_count / 2;
    let mut rng = ChaCha8Rng::seed_from_u64(recipe.seed);

    for _ in 0..MAX_PLAN_ATTEMPTS {
        if let Some(pieces) = plan_track(piece_count, checkpoint_index, &mut rng) {
            return pieces;
        }
    }

    panic!(
        "track planner failed after {MAX_PLAN_ATTEMPTS} attempts for seed {}",
        recipe.seed
    )
}

fn plan_track(
    piece_count: usize,
    checkpoint_index: usize,
    rng: &mut ChaCha8Rng,
) -> Option<Vec<TrackPiece>> {
    let start = Pose2::new(
        Vec2::new(0.0, -((piece_count as f32) * PIECE_LENGTH * 0.5)),
        0.0,
    );
    let mut stack = Vec::with_capacity(piece_count);
    let mut occupied = HashSet::new();

    stack.push(PlanStep::new(0, start, piece_count, checkpoint_index, rng));

    while let Some(mut step) = stack.pop() {
        if step.index == piece_count {
            return Some(stack.into_iter().filter_map(|step| step.piece).collect());
        }

        if let Some(piece) = step.piece.take() {
            for cell in occupied_cells(&piece) {
                occupied.remove(&cell);
            }
        }

        if let Some(piece) = step.next_fit(&occupied, rng) {
            let next_entry = piece.exit();
            for cell in occupied_cells(&piece) {
                occupied.insert(cell);
            }
            step.piece = Some(piece);
            let next_index = step.index + 1;
            stack.push(step);
            stack.push(PlanStep::new(
                next_index,
                next_entry,
                piece_count,
                checkpoint_index,
                rng,
            ));
        }
    }

    None
}

struct PlanStep {
    index: usize,
    entry: Pose2,
    candidates: Vec<TrackPieceKind>,
    next_candidate: usize,
    piece: Option<TrackPiece>,
}

impl PlanStep {
    fn new(
        index: usize,
        entry: Pose2,
        piece_count: usize,
        checkpoint_index: usize,
        rng: &mut ChaCha8Rng,
    ) -> Self {
        Self {
            index,
            entry,
            candidates: piece_kind_candidates(entry, index, piece_count, checkpoint_index, rng),
            next_candidate: 0,
            piece: None,
        }
    }

    fn next_fit(
        &mut self,
        occupied: &HashSet<super::validation::OccupancyCell>,
        rng: &mut ChaCha8Rng,
    ) -> Option<TrackPiece> {
        while let Some(kind) = self.candidates.get(self.next_candidate).copied() {
            self.next_candidate += 1;
            let surface = match kind {
                TrackPieceKind::Finish => SurfaceKind::Boost,
                _ => generated_surface(rng),
            };
            let piece = TrackPiece {
                kind,
                surface,
                frames: generated_frames(self.entry, kind),
            };

            if candidate_is_valid(occupied, &piece, self.index).is_ok() {
                return Some(piece);
            }
        }

        None
    }
}

fn piece_kind_candidates(
    entry: Pose2,
    index: usize,
    piece_count: usize,
    checkpoint_index: usize,
    rng: &mut ChaCha8Rng,
) -> Vec<TrackPieceKind> {
    if let Some(required) = required_piece_kind(index, piece_count, checkpoint_index) {
        return vec![required];
    }

    let primary = curve_direction(entry.yaw, rng);
    let secondary = match primary {
        TurnDirection::Left => TurnDirection::Right,
        TurnDirection::Right => TurnDirection::Left,
    };

    vec![
        TrackPieceKind::Curve(primary),
        TrackPieceKind::Straight,
        TrackPieceKind::Curve(secondary),
    ]
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

fn curve_direction(route_yaw: f32, rng: &mut ChaCha8Rng) -> TurnDirection {
    if route_yaw > 0.2 {
        return TurnDirection::Left;
    }
    if route_yaw < -0.2 {
        return TurnDirection::Right;
    }

    if rng.random_bool(0.5) {
        TurnDirection::Right
    } else {
        TurnDirection::Left
    }
}

fn generated_surface(rng: &mut ChaCha8Rng) -> SurfaceKind {
    match rng.random_range(0..12) {
        0..=6 => SurfaceKind::Asphalt,
        7..=8 => SurfaceKind::Dirt,
        9..=10 => SurfaceKind::Ice,
        _ => SurfaceKind::Boost,
    }
}
