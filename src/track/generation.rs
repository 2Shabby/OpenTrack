use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::driving::CarSpawn;
use crate::spatial::Pose2;
use crate::surface::SurfaceKind;

pub const TRACK_WIDTH: f32 = 12.0;
pub const PIECE_LENGTH: f32 = 14.0;
pub const RAIL_HEIGHT: f32 = 0.45;
pub const RAIL_THICKNESS: f32 = 0.28;

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
    pub road_surface_count: usize,
    pub rail_count: usize,
    pub trigger_count: usize,
}

impl GeneratedTrackInfo {
    pub fn from_pieces(recipe: &TrackRecipe, pieces: &[TrackPiece]) -> Self {
        Self {
            seed: recipe.seed,
            piece_count: pieces.len(),
            checkpoint_count: TrackPiece::checkpoint_count(pieces),
            road_surface_count: pieces.len(),
            rail_count: pieces.len() * 2,
            trigger_count: TrackPiece::trigger_count(pieces),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TrackPieceKind {
    Straight,
    Checkpoint(usize),
    Finish,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackPiece {
    pub kind: TrackPieceKind,
    pub surface: SurfaceKind,
    pub pose: Pose2,
    pub entry: Pose2,
    pub exit: Pose2,
}

impl TrackPiece {
    pub fn bounds(self) -> Vec2 {
        Vec2::new(TRACK_WIDTH * 0.5, PIECE_LENGTH * 0.5)
    }

    pub fn checkpoint_count(pieces: &[Self]) -> usize {
        pieces
            .iter()
            .filter(|piece| matches!(piece.kind, TrackPieceKind::Checkpoint(_)))
            .count()
    }

    pub fn trigger_count(pieces: &[Self]) -> usize {
        pieces
            .iter()
            .filter(|piece| {
                matches!(
                    piece.kind,
                    TrackPieceKind::Checkpoint(_) | TrackPieceKind::Finish
                )
            })
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

    (0..piece_count)
        .map(|index| {
            let piece_yaw = entry.yaw;
            let exit = Pose2::new(
                entry.position + Pose2::new(Vec2::ZERO, piece_yaw).forward() * PIECE_LENGTH,
                entry.yaw,
            );
            let kind = piece_kind(index, piece_count, checkpoint_index);
            let surface = match kind {
                TrackPieceKind::Finish => SurfaceKind::Boost,
                _ => generated_surface(&mut rng),
            };
            let piece = TrackPiece {
                kind,
                surface,
                pose: Pose2::new(
                    Vec2::new(
                        (entry.position.x + exit.position.x) * 0.5,
                        (entry.position.y + exit.position.y) * 0.5,
                    ),
                    piece_yaw,
                ),
                entry,
                exit,
            };

            entry = exit;
            piece
        })
        .collect()
}

pub fn validate_piece_connections(pieces: &[TrackPiece]) -> Result<(), String> {
    for (index, pair) in pieces.windows(2).enumerate() {
        let previous = pair[0];
        let next = pair[1];
        let gap = previous.exit.position.distance(next.entry.position);
        let yaw_delta = (previous.exit.yaw - next.entry.yaw).abs();

        if gap > 0.001 || yaw_delta > 0.001 {
            return Err(format!(
                "piece {} -> {} has gap {:.4} and yaw delta {:.4}",
                index,
                index + 1,
                gap,
                yaw_delta
            ));
        }
    }

    Ok(())
}

pub fn car_spawn_for(pieces: &[TrackPiece]) -> CarSpawn {
    let Some(first_piece) = pieces.first() else {
        return CarSpawn::default();
    };
    let start = first_piece.entry.position + forward_2d(first_piece.entry.yaw) * 1.1;

    CarSpawn {
        translation: Vec3::new(start.x, 0.05, start.y),
        yaw: first_piece.entry.yaw,
    }
}

pub fn forward_2d(yaw: f32) -> Vec2 {
    Pose2::new(Vec2::ZERO, yaw).forward()
}

fn piece_kind(index: usize, piece_count: usize, checkpoint_index: usize) -> TrackPieceKind {
    if index == piece_count - 1 {
        TrackPieceKind::Finish
    } else if index == checkpoint_index {
        TrackPieceKind::Checkpoint(0)
    } else {
        TrackPieceKind::Straight
    }
}

fn generated_surface(rng: &mut ChaCha8Rng) -> SurfaceKind {
    match rng.random_range(0..10) {
        0..=5 => SurfaceKind::Asphalt,
        6..=7 => SurfaceKind::Dirt,
        8 => SurfaceKind::Ice,
        _ => SurfaceKind::Boost,
    }
}
