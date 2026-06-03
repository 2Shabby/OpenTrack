use bevy::prelude::*;

use super::types::{PIECE_LENGTH, PathFrame, TrackPieceKind, TurnDirection};
use crate::geometry::{Pose2, rotate_2d};

const CURVE_RADIUS: f32 = 24.0;
const CURVE_STEPS: usize = 6;

#[derive(Clone, Copy, Debug)]
pub(crate) enum TrackPath {
    Straight {
        entry: Pose2,
        length: f32,
    },
    ConstantArc {
        entry: Pose2,
        direction: TurnDirection,
        radius: f32,
        angle: f32,
        steps: usize,
    },
}

impl TrackPath {
    pub(crate) fn for_piece(entry: Pose2, kind: TrackPieceKind, difficulty: u8) -> Self {
        match kind {
            TrackPieceKind::Straight | TrackPieceKind::Checkpoint(_) | TrackPieceKind::Finish => {
                Self::Straight {
                    entry,
                    length: PIECE_LENGTH,
                }
            }
            TrackPieceKind::Curve(direction) => Self::ConstantArc {
                entry,
                direction,
                radius: CURVE_RADIUS,
                angle: curve_angle(difficulty),
                steps: CURVE_STEPS,
            },
        }
    }

    pub(crate) fn sample_frames(self) -> Vec<PathFrame> {
        match self {
            Self::Straight { entry, length } => straight_frames(entry, length),
            Self::ConstantArc {
                entry,
                direction,
                radius,
                angle,
                steps,
            } => arc_frames(entry, direction, radius, angle, steps),
        }
    }
}

pub(crate) fn generated_frames(
    entry: Pose2,
    kind: TrackPieceKind,
    difficulty: u8,
) -> Vec<PathFrame> {
    TrackPath::for_piece(entry, kind, difficulty).sample_frames()
}

fn straight_frames(entry: Pose2, length: f32) -> Vec<PathFrame> {
    let exit = Pose2::new(entry.position + entry.forward() * length, entry.yaw);
    vec![PathFrame { pose: entry }, PathFrame { pose: exit }]
}

fn arc_frames(
    entry: Pose2,
    direction: TurnDirection,
    radius: f32,
    angle: f32,
    steps: usize,
) -> Vec<PathFrame> {
    let side = direction.side();
    let center = entry.position + entry.right() * side * radius;
    let radius_vector = -entry.right() * side * radius;

    (0..=steps)
        .map(|step| {
            let t = step as f32 / steps as f32;
            let rotation = side * angle * t;
            PathFrame {
                pose: Pose2::new(
                    center + rotate_2d(radius_vector, rotation),
                    entry.yaw + side * angle * t,
                ),
            }
        })
        .collect()
}

fn curve_angle(difficulty: u8) -> f32 {
    match difficulty {
        0 => std::f32::consts::FRAC_PI_6,
        1 => std::f32::consts::FRAC_PI_4,
        _ => std::f32::consts::FRAC_PI_3,
    }
}
