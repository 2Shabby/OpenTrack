use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::driving::CarSpawn;
use crate::spatial::{Pose2, rotate_2d};
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
            road_surface_count: pieces.iter().map(TrackPiece::segment_count).sum(),
            rail_count: pieces.iter().map(TrackPiece::segment_count).sum::<usize>() * 2,
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

#[derive(Clone, Copy, Debug)]
pub struct TrackSegment {
    pub pose: Pose2,
    pub length: f32,
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

    pub fn segments(&self) -> Vec<TrackSegment> {
        self.frames
            .windows(2)
            .map(|pair| {
                let entry = pair[0].pose;
                let exit = pair[1].pose;
                TrackSegment {
                    pose: Pose2::new(
                        (entry.position + exit.position) * 0.5,
                        mid_yaw(entry.yaw, exit.yaw),
                    ),
                    length: entry.position.distance(exit.position),
                }
            })
            .collect()
    }

    pub fn segment_count(&self) -> usize {
        self.frames.len().saturating_sub(1)
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
            let kind = piece_kind(index, piece_count, checkpoint_index);
            let surface = match kind {
                TrackPieceKind::Finish => SurfaceKind::Boost,
                _ => generated_surface(&mut rng),
            };
            let frames = generated_frames(entry, kind, &mut rng);
            let exit = frames.last().map(|frame| frame.pose).unwrap_or(entry);
            let piece = TrackPiece {
                kind,
                surface,
                frames,
            };

            entry = exit;
            piece
        })
        .collect()
}

pub fn validate_piece_connections(pieces: &[TrackPiece]) -> Result<(), String> {
    for (index, pair) in pieces.windows(2).enumerate() {
        let previous = &pair[0];
        let next = &pair[1];
        let gap = previous.exit().position.distance(next.entry().position);
        let yaw_delta = (previous.exit().yaw - next.entry().yaw).abs();

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

fn generated_frames(entry: Pose2, kind: TrackPieceKind, rng: &mut ChaCha8Rng) -> Vec<PathFrame> {
    if matches!(kind, TrackPieceKind::Checkpoint(_) | TrackPieceKind::Finish)
        || rng.random_range(0..10) < 6
    {
        return straight_frames(entry);
    }

    let side = if rng.random_bool(0.5) { 1.0 } else { -1.0 };
    curve_frames(entry, side)
}

fn straight_frames(entry: Pose2) -> Vec<PathFrame> {
    let exit = Pose2::new(entry.position + entry.forward() * PIECE_LENGTH, entry.yaw);
    vec![PathFrame { pose: entry }, PathFrame { pose: exit }]
}

fn curve_frames(entry: Pose2, side: f32) -> Vec<PathFrame> {
    const CURVE_RADIUS: f32 = 24.0;
    const CURVE_ANGLE: f32 = std::f32::consts::FRAC_PI_4;
    const CURVE_STEPS: usize = 6;

    let center = entry.position + entry.right() * side * CURVE_RADIUS;
    let radius_vector = -entry.right() * side * CURVE_RADIUS;

    (0..=CURVE_STEPS)
        .map(|step| {
            let t = step as f32 / CURVE_STEPS as f32;
            let angle = side * CURVE_ANGLE * t;
            PathFrame {
                pose: Pose2::new(
                    center + rotate_2d(radius_vector, angle),
                    entry.yaw + side * CURVE_ANGLE * t,
                ),
            }
        })
        .collect()
}

fn mid_yaw(entry_yaw: f32, exit_yaw: f32) -> f32 {
    entry_yaw + (exit_yaw - entry_yaw) * 0.5
}
