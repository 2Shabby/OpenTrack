use bevy::prelude::*;
use kurbo::{BezPath, PathEl, Point};

use super::generation::PathFrame;

#[derive(Clone, Debug)]
pub struct RoadEdges {
    pub left: Vec<Vec2>,
    pub right: Vec<Vec2>,
}

pub fn road_edges(frames: &[PathFrame], width: f32) -> RoadEdges {
    let half_width = width * 0.5;
    let mut left = Vec::with_capacity(frames.len());
    let mut right = Vec::with_capacity(frames.len());

    for frame in frames {
        let frame_right = frame.pose.right();
        left.push(frame.pose.position - frame_right * half_width);
        right.push(frame.pose.position + frame_right * half_width);
    }

    RoadEdges { left, right }
}

pub fn road_polygon(frames: &[PathFrame], width: f32) -> BezPath {
    let edges = road_edges(frames, width);
    let mut path = BezPath::new();

    let Some(first_left) = edges.left.first().copied() else {
        return path;
    };

    path.move_to(point(first_left));
    for right_edge in edges.right {
        path.line_to(point(right_edge));
    }
    for left_edge in edges.left.into_iter().rev() {
        path.line_to(point(left_edge));
    }
    path.close_path();

    path
}

pub fn line_path(points: &[Vec2]) -> BezPath {
    let mut path = BezPath::new();
    let Some(first) = points.first().copied() else {
        return path;
    };

    path.move_to(point(first));
    for next_point in points.iter().copied().skip(1) {
        path.line_to(point(next_point));
    }

    path
}

pub fn line_segments(path: &BezPath) -> Vec<[Vec2; 2]> {
    let mut segments = Vec::new();
    let mut previous = None;

    for element in path.elements() {
        match *element {
            PathEl::MoveTo(point) => previous = Some(vec2(point)),
            PathEl::LineTo(point) => {
                let current = vec2(point);
                if let Some(previous) = previous {
                    segments.push([previous, current]);
                }
                previous = Some(current);
            }
            PathEl::ClosePath => previous = None,
            PathEl::QuadTo(_, _) | PathEl::CurveTo(_, _, _) => {
                previous = None;
            }
        }
    }

    segments
}

pub fn polygon_points(path: &BezPath) -> Vec<Vec2> {
    path.elements()
        .iter()
        .filter_map(|element| match *element {
            PathEl::MoveTo(point) | PathEl::LineTo(point) => Some(vec2(point)),
            PathEl::ClosePath | PathEl::QuadTo(_, _) | PathEl::CurveTo(_, _, _) => None,
        })
        .collect()
}

fn point(value: Vec2) -> Point {
    Point::new(value.x as f64, value.y as f64)
}

fn vec2(value: Point) -> Vec2 {
    Vec2::new(value.x as f32, value.y as f32)
}
