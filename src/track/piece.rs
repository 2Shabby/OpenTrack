use bevy::prelude::*;

use super::generation::{PathFrame, RAIL_THICKNESS, TRACK_WIDTH, TrackPiece, TrackPieceKind};
use super::path_geometry::road_edges;
use crate::geometry::{OrientedRect, Pose2};

#[derive(Clone, Debug)]
pub struct TrackPieceGeometry {
    pub roads: Vec<TrackRoadSpan>,
    pub rails: Vec<TrackRailSpan>,
    pub trigger: Option<TrackTriggerLine>,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackRoadSpan {
    pub bounds: OrientedRect,
    pub length: f32,
}

#[derive(Clone, Debug)]
pub struct TrackRailSpan {
    pub points: Vec<Vec2>,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackTriggerLine {
    pub marker: TrackPieceMarker,
    pub bounds: OrientedRect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackPieceMarker {
    Checkpoint(usize),
    Finish,
}

impl TrackPiece {
    pub fn geometry(&self) -> TrackPieceGeometry {
        let roads: Vec<_> = self
            .frames
            .windows(2)
            .map(|pair| road_span([pair[0], pair[1]]))
            .collect();
        let edges = road_edges(&self.frames, TRACK_WIDTH + RAIL_THICKNESS);
        let rails = vec![
            TrackRailSpan { points: edges.left },
            TrackRailSpan {
                points: edges.right,
            },
        ];

        TrackPieceGeometry {
            roads,
            rails,
            trigger: trigger_line(self),
        }
    }

    pub fn rail_count(pieces: &[Self]) -> usize {
        if pieces.is_empty() { 0 } else { 2 }
    }

    pub fn trigger_count(pieces: &[Self]) -> usize {
        pieces
            .iter()
            .filter(|piece| trigger_line(piece).is_some())
            .count()
    }
}

fn road_span(frames: [PathFrame; 2]) -> TrackRoadSpan {
    let entry = frames[0].pose;
    let exit = frames[1].pose;
    let length = entry.position.distance(exit.position);
    let pose = Pose2::new(
        (entry.position + exit.position) * 0.5,
        midpoint_yaw(entry.yaw, exit.yaw),
    );

    TrackRoadSpan {
        bounds: OrientedRect::new(pose, Vec2::new(TRACK_WIDTH * 0.5, length * 0.5)),
        length,
    }
}

fn trigger_line(piece: &TrackPiece) -> Option<TrackTriggerLine> {
    let (marker, pose) = match piece.kind {
        TrackPieceKind::Straight | TrackPieceKind::Turn { .. } => return None,
        TrackPieceKind::Checkpoint(index) => (TrackPieceMarker::Checkpoint(index), piece.entry()),
        TrackPieceKind::Finish => (TrackPieceMarker::Finish, piece.exit()),
    };

    Some(TrackTriggerLine {
        marker,
        bounds: OrientedRect::new(pose, Vec2::new(TRACK_WIDTH * 0.5, 0.45)),
    })
}

fn midpoint_yaw(entry_yaw: f32, exit_yaw: f32) -> f32 {
    entry_yaw + (exit_yaw - entry_yaw) * 0.5
}
