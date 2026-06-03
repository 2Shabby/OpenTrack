use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::path::generated_frames;
use super::types::{
    PIECE_LENGTH, SurfaceMix, TrackPiece, TrackPieceKind, TrackRecipe, TurnDirection,
};
use super::validation::candidate_is_valid;
use crate::geometry::Pose2;
use crate::surface::SurfaceKind;

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
