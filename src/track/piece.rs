use bevy::prelude::*;

use super::generation::{PathFrame, RAIL_THICKNESS, TRACK_WIDTH, TrackPiece, TrackPieceKind};
use crate::spatial::{OrientedRect, Pose2};
use crate::surface::SurfaceKind;

#[derive(Clone, Debug)]
pub struct TrackPieceGeometry {
    pub roads: Vec<TrackRoadSpan>,
    pub rails: Vec<TrackRailSpan>,
    pub trigger: Option<TrackTriggerLine>,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackRoadSpan {
    pub frames: [PathFrame; 2],
    pub surface: SurfaceKind,
    pub bounds: OrientedRect,
    pub length: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackRailSpan {
    pub bounds: OrientedRect,
    pub length: f32,
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
            .map(|pair| road_span([pair[0], pair[1]], self.surface))
            .collect();
        let rails = roads
            .iter()
            .flat_map(|road| [-1.0, 1.0].map(|side| rail_span(*road, side)))
            .collect();

        TrackPieceGeometry {
            roads,
            rails,
            trigger: trigger_line(self),
        }
    }

    pub fn segment_count(&self) -> usize {
        self.frames.len().saturating_sub(1)
    }

    pub fn rail_count(&self) -> usize {
        self.segment_count() * 2
    }

    pub fn trigger_count(pieces: &[Self]) -> usize {
        pieces
            .iter()
            .filter(|piece| trigger_line(piece).is_some())
            .count()
    }
}

fn road_span(frames: [PathFrame; 2], surface: SurfaceKind) -> TrackRoadSpan {
    let entry = frames[0].pose;
    let exit = frames[1].pose;
    let length = entry.position.distance(exit.position);
    let pose = Pose2::new(
        (entry.position + exit.position) * 0.5,
        midpoint_yaw(entry.yaw, exit.yaw),
    );

    TrackRoadSpan {
        frames,
        surface,
        bounds: OrientedRect::new(pose, Vec2::new(TRACK_WIDTH * 0.5, length * 0.5)),
        length,
    }
}

fn rail_span(road: TrackRoadSpan, side: f32) -> TrackRailSpan {
    let local = Vec2::new(side * (TRACK_WIDTH * 0.5 + RAIL_THICKNESS * 0.5), 0.0);
    let pose = Pose2::new(road.bounds.pose.local_to_world(local), road.bounds.pose.yaw);

    TrackRailSpan {
        bounds: OrientedRect::new(pose, Vec2::new(RAIL_THICKNESS * 0.5, road.length * 0.5)),
        length: road.length,
    }
}

fn trigger_line(piece: &TrackPiece) -> Option<TrackTriggerLine> {
    let (marker, pose) = match piece.kind {
        TrackPieceKind::Straight => return None,
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
