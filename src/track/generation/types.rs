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
            rail_count: TrackPiece::rail_count(pieces),
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
        assert!(
            !pieces.is_empty(),
            "track bounds require at least one generated piece"
        );

        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);

        for frame in pieces.iter().flat_map(|piece| piece.frames.iter()) {
            min = min.min(frame.pose.position);
            max = max.max(frame.pose.position);
        }

        assert!(
            min.is_finite() && max.is_finite(),
            "track bounds require generated path frames"
        );

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
    Turn {
        direction: TurnDirection,
        angle: TurnAngle,
    },
    Checkpoint(usize),
    Finish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnAngle {
    Deg45,
    Deg90,
    Deg180,
}

impl TurnAngle {
    pub(crate) fn radians(self) -> f32 {
        match self {
            Self::Deg45 => std::f32::consts::FRAC_PI_4,
            Self::Deg90 => std::f32::consts::FRAC_PI_2,
            Self::Deg180 => std::f32::consts::PI,
        }
    }

    pub(crate) fn sample_steps(self) -> usize {
        match self {
            Self::Deg45 => 6,
            Self::Deg90 => 10,
            Self::Deg180 => 18,
        }
    }
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
            .expect("track pieces require at least one path frame")
    }

    pub fn exit(&self) -> Pose2 {
        self.frames
            .last()
            .map(|frame| frame.pose)
            .expect("track pieces require at least one path frame")
    }

    pub fn checkpoint_count(pieces: &[Self]) -> usize {
        pieces
            .iter()
            .filter(|piece| matches!(piece.kind, TrackPieceKind::Checkpoint(_)))
            .count()
    }
}

pub fn car_spawn_for(pieces: &[TrackPiece]) -> CarSpawn {
    let entry = pieces
        .first()
        .expect("car spawn requires at least one generated track piece")
        .entry();
    let start = entry.position + entry.forward() * 1.1;

    CarSpawn {
        translation: crate::geometry::xz_translation(start, 0.05),
        yaw: entry.yaw,
    }
}
