use bevy::prelude::*;

use crate::driving::{CAR_GROUND_OFFSET, CarSpawn};
use crate::geometry::{Pose2, forward_3d, right_3d, rotation_from_yaw_and_up, xz_translation};
use crate::surface::SurfaceKind;

pub const TRACK_WIDTH: f32 = 12.0;
pub const PIECE_LENGTH: f32 = 14.0;
pub const RAIL_HEIGHT: f32 = 0.45;
pub const RAIL_THICKNESS: f32 = 0.28;
pub const MAX_BANK_ANGLE: f32 = std::f32::consts::FRAC_PI_4;

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
pub enum TrackPieceKind {
    Straight,
    DoubleStraight,
    BankTransition {
        direction: TurnDirection,
        angle: BankAngle,
        mode: BankTransitionMode,
    },
    BankedStraight {
        direction: TurnDirection,
        angle: BankAngle,
    },
    BankedDoubleStraight {
        direction: TurnDirection,
        angle: BankAngle,
    },
    BankedTurn {
        direction: TurnDirection,
        turn_angle: TurnAngle,
        bank_angle: BankAngle,
    },
    Turn {
        direction: TurnDirection,
        angle: TurnAngle,
    },
    Checkpoint(usize),
    Finish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BankAngle {
    Deg30,
    Deg45,
}

impl BankAngle {
    pub(crate) fn radians(self) -> f32 {
        match self {
            Self::Deg30 => std::f32::consts::FRAC_PI_6,
            Self::Deg45 => MAX_BANK_ANGLE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BankTransitionMode {
    In,
    Out,
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
    pub position: Vec2,
    pub yaw: f32,
    pub bank: f32,
    pub center: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackConnector {
    pub position: Vec2,
    pub yaw: f32,
    pub bank: f32,
}

impl TrackConnector {
    pub fn new(position: Vec2, yaw: f32, bank: f32) -> Self {
        assert!(
            bank.abs() <= MAX_BANK_ANGLE + 0.001,
            "track bank exceeds 45 degrees"
        );
        Self {
            position,
            yaw,
            bank,
        }
    }

    pub fn pose(self) -> Pose2 {
        Pose2::new(self.position, self.yaw)
    }

    pub fn forward(self) -> Vec2 {
        self.pose().forward()
    }

    pub fn right(self) -> Vec2 {
        self.pose().right()
    }
}

impl From<Pose2> for TrackConnector {
    fn from(pose: Pose2) -> Self {
        Self::new(pose.position, pose.yaw, 0.0)
    }
}

impl PathFrame {
    pub fn new(position: Vec2, yaw: f32, bank: f32) -> Self {
        let connector = TrackConnector::new(position, yaw, bank);
        let flat_forward = forward_3d(yaw);
        let flat_right = right_3d(yaw);
        let (bank_sin, bank_cos) = bank.sin_cos();
        let right = (flat_right * bank_cos - Vec3::Y * bank_sin).normalize();
        let normal = flat_forward.cross(right).normalize();

        Self {
            position: connector.position,
            yaw: connector.yaw,
            bank: connector.bank,
            center: xz_translation(connector.position, 0.0),
            forward: flat_forward,
            right,
            normal,
        }
    }

    pub fn connector(self) -> TrackConnector {
        TrackConnector::new(self.position, self.yaw, self.bank)
    }

    pub fn surface_transform(self, normal_offset: f32) -> Transform {
        Transform::from_translation(self.center + self.normal * normal_offset)
            .with_rotation(rotation_from_yaw_and_up(self.yaw, self.normal))
    }
}

impl TrackPiece {
    pub fn entry(&self) -> TrackConnector {
        self.frames
            .first()
            .map(|frame| frame.connector())
            .expect("track pieces require at least one path frame")
    }

    pub fn exit(&self) -> TrackConnector {
        self.frames
            .last()
            .map(|frame| frame.connector())
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
    let frame = PathFrame::new(start, entry.yaw, entry.bank);

    CarSpawn {
        translation: frame.center + frame.normal * CAR_GROUND_OFFSET,
        yaw: entry.yaw,
        up: frame.normal,
    }
}
