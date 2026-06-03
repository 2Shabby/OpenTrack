use bevy::math::curve::{Curve, FunctionCurve, Interval};
use bevy::prelude::*;

use super::types::{
    BankAngle, BankTransitionMode, PIECE_LENGTH, PathFrame, TrackConnector, TrackPieceKind,
    TurnDirection,
};
use crate::geometry::{Pose2, rotate_2d};

const CURVE_RADIUS: f32 = 24.0;
const BANK_EPSILON: f32 = 0.001;
const BANK_TRANSITION_SEGMENT_LENGTH: f32 = 1.75;
const MIN_BANK_TRANSITION_STEPS: usize = 8;

#[derive(Clone, Copy, Debug)]
pub(crate) enum TrackPath {
    Straight {
        entry: TrackConnector,
        length: f32,
        exit_bank: f32,
    },
    ConstantArc {
        entry: TrackConnector,
        direction: TurnDirection,
        radius: f32,
        angle: f32,
        steps: usize,
        exit_bank: f32,
    },
}

impl TrackPath {
    pub(crate) fn for_piece(entry: TrackConnector, kind: TrackPieceKind) -> Self {
        match kind {
            TrackPieceKind::Straight | TrackPieceKind::Checkpoint(_) | TrackPieceKind::Finish => {
                Self::straight(entry, PIECE_LENGTH, entry.bank)
            }
            TrackPieceKind::DoubleStraight => Self::straight(entry, PIECE_LENGTH * 2.0, entry.bank),
            TrackPieceKind::BankTransition {
                direction,
                angle,
                mode,
            } => {
                let exit_bank = match mode {
                    BankTransitionMode::In => signed_bank(direction, angle),
                    BankTransitionMode::Out => 0.0,
                };
                Self::straight(entry, PIECE_LENGTH, exit_bank)
            }
            TrackPieceKind::BankedStraight { direction, angle } => {
                Self::straight(entry, PIECE_LENGTH, signed_bank(direction, angle))
            }
            TrackPieceKind::BankedDoubleStraight { direction, angle } => {
                Self::straight(entry, PIECE_LENGTH * 2.0, signed_bank(direction, angle))
            }
            TrackPieceKind::Turn { direction, angle } => Self::ConstantArc {
                entry,
                direction,
                radius: CURVE_RADIUS,
                angle: angle.radians(),
                steps: angle.sample_steps(),
                exit_bank: entry.bank,
            },
            TrackPieceKind::BankedTurn {
                direction,
                turn_angle,
                bank_angle,
            } => Self::ConstantArc {
                entry,
                direction,
                radius: CURVE_RADIUS,
                angle: turn_angle.radians(),
                steps: turn_angle.sample_steps(),
                exit_bank: signed_bank(direction, bank_angle),
            },
        }
    }

    fn straight(entry: TrackConnector, length: f32, exit_bank: f32) -> Self {
        Self::Straight {
            entry,
            length,
            exit_bank,
        }
    }

    pub(crate) fn sample_frames(self) -> Vec<PathFrame> {
        match self {
            Self::Straight {
                entry,
                length,
                exit_bank,
            } => straight_frames(entry, length, exit_bank),
            Self::ConstantArc {
                entry,
                direction,
                radius,
                angle,
                steps,
                exit_bank,
            } => arc_frames(entry, direction, radius, angle, steps, exit_bank),
        }
    }
}

pub(crate) fn generated_frames(entry: TrackConnector, kind: TrackPieceKind) -> Vec<PathFrame> {
    TrackPath::for_piece(entry, kind).sample_frames()
}

fn straight_frames(entry: TrackConnector, length: f32, exit_bank: f32) -> Vec<PathFrame> {
    let curve = FunctionCurve::new(Interval::UNIT, move |t| {
        Pose2::new(entry.position + entry.forward() * length * t, entry.yaw)
    });

    sample_pose_curve(
        &curve,
        straight_sample_steps(length, entry.bank, exit_bank),
        entry.bank,
        exit_bank,
    )
}

fn arc_frames(
    entry: TrackConnector,
    direction: TurnDirection,
    radius: f32,
    angle: f32,
    steps: usize,
    exit_bank: f32,
) -> Vec<PathFrame> {
    let side = direction.side();
    let entry_pose = entry.pose();
    let center = entry.position + entry_pose.right() * side * radius;
    let radius_vector = -entry_pose.right() * side * radius;
    let curve = FunctionCurve::new(Interval::UNIT, move |t| {
        let rotation = side * angle * t;
        Pose2::new(
            center + rotate_2d(radius_vector, -rotation),
            entry.yaw + rotation,
        )
    });

    sample_pose_curve(&curve, steps, entry.bank, exit_bank)
}

fn sample_pose_curve(
    curve: &impl Curve<Pose2>,
    steps: usize,
    entry_bank: f32,
    exit_bank: f32,
) -> Vec<PathFrame> {
    let steps = steps.max(1);

    (0..=steps)
        .map(|step| {
            let t = step as f32 / steps as f32;
            let pose = curve
                .sample(t)
                .expect("unit-interval samples must be valid");
            let bank = entry_bank + (exit_bank - entry_bank) * smoothstep(t);
            PathFrame::new(pose.position, pose.yaw, bank)
        })
        .collect()
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn straight_sample_steps(length: f32, entry_bank: f32, exit_bank: f32) -> usize {
    if (exit_bank - entry_bank).abs() <= BANK_EPSILON {
        return 1;
    }

    ((length / BANK_TRANSITION_SEGMENT_LENGTH).ceil() as usize).max(MIN_BANK_TRANSITION_STEPS)
}

fn signed_bank(direction: TurnDirection, angle: BankAngle) -> f32 {
    direction.side() * angle.radians()
}
