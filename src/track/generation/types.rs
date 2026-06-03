use bevy::prelude::*;

use crate::driving::CarSpawn;
use crate::geometry::Pose2;
use crate::surface::SurfaceKind;

pub const TRACK_WIDTH: f32 = 12.0;
pub const PIECE_LENGTH: f32 = 14.0;
pub const RAIL_HEIGHT: f32 = 0.45;
pub const RAIL_THICKNESS: f32 = 0.28;

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
    pub(crate) fn side(self) -> f32 {
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
