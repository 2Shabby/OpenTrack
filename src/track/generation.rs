use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::driving::CarSpawn;
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
}

#[derive(Clone, Copy, Debug)]
pub enum TrackPieceKind {
    Straight,
    Curve,
    Checkpoint(usize),
    Finish,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackPiece {
    pub kind: TrackPieceKind,
    pub surface: SurfaceKind,
    pub center: Vec3,
    pub yaw: f32,
    pub entry: Transform2d,
    pub exit: Transform2d,
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
}

#[derive(Clone, Copy, Debug)]
pub struct Transform2d {
    pub position: Vec2,
    pub yaw: f32,
}

pub fn generate_track_pieces(recipe: &TrackRecipe) -> Vec<TrackPiece> {
    let piece_count = recipe.piece_count.max(4);
    let checkpoint_index = piece_count / 2;
    let mut rng = ChaCha8Rng::seed_from_u64(recipe.seed);
    let mut entry = Transform2d {
        position: Vec2::new(0.0, -((piece_count as f32) * PIECE_LENGTH * 0.5)),
        yaw: 0.0,
    };

    (0..piece_count)
        .map(|index| {
            let yaw_delta = if index > 0 && index < piece_count - 1 {
                generated_yaw_delta(&mut rng)
            } else {
                0.0
            };
            let exit_yaw = (entry.yaw + yaw_delta).clamp(-0.55, 0.55);
            let piece_yaw = average_angle(entry.yaw, exit_yaw);
            let exit = Transform2d {
                position: entry.position + forward_2d(piece_yaw) * PIECE_LENGTH,
                yaw: exit_yaw,
            };
            let kind = piece_kind(index, piece_count, checkpoint_index, yaw_delta);
            let surface = match kind {
                TrackPieceKind::Finish => SurfaceKind::Boost,
                _ => generated_surface(&mut rng),
            };
            let piece = TrackPiece {
                kind,
                surface,
                center: Vec3::new(
                    (entry.position.x + exit.position.x) * 0.5,
                    0.0,
                    (entry.position.y + exit.position.y) * 0.5,
                ),
                yaw: piece_yaw,
                entry,
                exit,
            };

            entry = exit;
            piece
        })
        .collect()
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
    Vec2::new(yaw.sin(), yaw.cos())
}

pub fn forward_3d(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

pub fn rotate_2d(value: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(value.x * cos - value.y * sin, value.x * sin + value.y * cos)
}

fn piece_kind(
    index: usize,
    piece_count: usize,
    checkpoint_index: usize,
    yaw_delta: f32,
) -> TrackPieceKind {
    if index == piece_count - 1 {
        TrackPieceKind::Finish
    } else if index == checkpoint_index {
        TrackPieceKind::Checkpoint(0)
    } else if yaw_delta.abs() > 0.01 {
        TrackPieceKind::Curve
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

fn generated_yaw_delta(rng: &mut ChaCha8Rng) -> f32 {
    match rng.random_range(0..5) {
        0 => -0.18,
        1 => 0.18,
        _ => 0.0,
    }
}

fn average_angle(a: f32, b: f32) -> f32 {
    let x = a.cos() + b.cos();
    let y = a.sin() + b.sin();
    y.atan2(x)
}
