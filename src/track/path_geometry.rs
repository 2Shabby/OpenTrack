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
    assert!(
        !frames.is_empty(),
        "road polygon requires at least one path frame"
    );

    let edges = road_edges(frames, width);
    let mut path = BezPath::new();

    path.move_to(point(edges.left[0]));
    for left_edge in edges.left.into_iter().skip(1) {
        path.line_to(point(left_edge));
    }
    for right_edge in edges.right.into_iter().rev() {
        path.line_to(point(right_edge));
    }
    path.close_path();

    path
}

pub fn line_path(points: &[Vec2]) -> BezPath {
    assert!(
        !points.is_empty(),
        "line path requires at least one generated point"
    );

    let mut path = BezPath::new();

    path.move_to(point(points[0]));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Pose2, rotate_2d};

    const TEST_RADIUS: f32 = 24.0;
    const TEST_WIDTH: f32 = 12.0;
    const TEST_STEPS: usize = 10;

    #[test]
    fn road_edges_keep_fixed_width_on_left_and_right_turns() {
        for direction in [Turn::Left, Turn::Right] {
            let frames = quarter_turn_frames(direction);
            let edges = road_edges(&frames, TEST_WIDTH);

            for ((frame, left), right) in frames.iter().zip(&edges.left).zip(&edges.right) {
                assert_approx(left.distance(*right), TEST_WIDTH);
                assert_approx(((*left + *right) * 0.5).distance(frame.pose.position), 0.0);
            }
        }
    }

    #[test]
    fn road_polygon_uses_left_forward_right_backward_order_for_both_turns() {
        for direction in [Turn::Left, Turn::Right] {
            let frames = quarter_turn_frames(direction);
            let edges = road_edges(&frames, TEST_WIDTH);
            let points = polygon_points(&road_polygon(&frames, TEST_WIDTH));

            assert_eq!(points.len(), frames.len() * 2);
            assert_points_match(&points[..edges.left.len()], &edges.left);

            let right_reversed = edges.right.into_iter().rev().collect::<Vec<_>>();
            assert_points_match(&points[edges.left.len()..], &right_reversed);
        }
    }

    #[test]
    fn turn_boundaries_do_not_cross_for_left_or_right_turns() {
        for direction in [Turn::Left, Turn::Right] {
            let frames = quarter_turn_frames(direction);
            let edges = road_edges(&frames, TEST_WIDTH);
            let center = arc_center(direction);
            let half_width = TEST_WIDTH * 0.5;

            for (left, right) in edges.left.iter().zip(&edges.right) {
                let left_radius = left.distance(center);
                let right_radius = right.distance(center);

                match direction {
                    Turn::Left => {
                        assert_approx(left_radius, TEST_RADIUS - half_width);
                        assert_approx(right_radius, TEST_RADIUS + half_width);
                    }
                    Turn::Right => {
                        assert_approx(left_radius, TEST_RADIUS + half_width);
                        assert_approx(right_radius, TEST_RADIUS - half_width);
                    }
                }
            }
        }
    }

    #[derive(Clone, Copy)]
    enum Turn {
        Left,
        Right,
    }

    impl Turn {
        fn side(self) -> f32 {
            match self {
                Self::Left => -1.0,
                Self::Right => 1.0,
            }
        }
    }

    fn quarter_turn_frames(direction: Turn) -> Vec<PathFrame> {
        let entry = Pose2::new(Vec2::ZERO, 0.0);
        let side = direction.side();
        let center = arc_center(direction);
        let radius_vector = -entry.right() * side * TEST_RADIUS;

        (0..=TEST_STEPS)
            .map(|step| {
                let t = step as f32 / TEST_STEPS as f32;
                let rotation = side * std::f32::consts::FRAC_PI_2 * t;
                PathFrame {
                    pose: Pose2::new(
                        center + rotate_2d(radius_vector, -rotation),
                        entry.yaw + rotation,
                    ),
                }
            })
            .collect()
    }

    fn arc_center(direction: Turn) -> Vec2 {
        Pose2::new(Vec2::ZERO, 0.0).right() * direction.side() * TEST_RADIUS
    }

    fn assert_points_match(actual: &[Vec2], expected: &[Vec2]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_approx(actual.distance(*expected), 0.0);
        }
    }

    fn assert_approx(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.001,
            "expected {expected:.4}, got {actual:.4}"
        );
    }
}
