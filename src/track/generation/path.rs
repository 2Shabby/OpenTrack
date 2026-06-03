use bevy::math::curve::{Curve, FunctionCurve, Interval};
use bevy::prelude::*;

use super::types::{PIECE_LENGTH, PathFrame, TrackPieceKind, TurnDirection};
use crate::geometry::{Pose2, rotate_2d};

const CURVE_RADIUS: f32 = 24.0;
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
    pub(crate) fn for_piece(entry: Pose2, kind: TrackPieceKind) -> Self {
        match kind {
            TrackPieceKind::Straight | TrackPieceKind::Checkpoint(_) | TrackPieceKind::Finish => {
                Self::straight(entry)
            }
            TrackPieceKind::Turn { direction, angle } => Self::ConstantArc {
                entry,
                direction,
                radius: CURVE_RADIUS,
                angle: angle.radians(),
                steps: angle.sample_steps(),
            },
        }
    }

    fn straight(entry: Pose2) -> Self {
        Self::Straight {
            entry,
            length: PIECE_LENGTH,
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

pub(crate) fn generated_frames(entry: Pose2, kind: TrackPieceKind) -> Vec<PathFrame> {
    TrackPath::for_piece(entry, kind).sample_frames()
}

fn straight_frames(entry: Pose2, length: f32) -> Vec<PathFrame> {
    let curve = FunctionCurve::new(Interval::UNIT, move |t| {
        Pose2::new(entry.position + entry.forward() * length * t, entry.yaw)
    });

    sample_pose_curve(&curve, 1)
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
    let curve = FunctionCurve::new(Interval::UNIT, move |t| {
        let rotation = side * angle * t;
        Pose2::new(
            center + rotate_2d(radius_vector, rotation),
            entry.yaw + side * angle * t,
        )
    });

    sample_pose_curve(&curve, steps)
}

fn sample_pose_curve(curve: &impl Curve<Pose2>, steps: usize) -> Vec<PathFrame> {
    let steps = steps.max(1);

    (0..=steps)
        .map(|step| {
            let t = step as f32 / steps as f32;
            PathFrame {
                pose: curve
                    .sample(t)
                    .expect("unit-interval samples must be valid"),
            }
        })
        .collect()
}
