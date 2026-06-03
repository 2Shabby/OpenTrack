use bevy::prelude::*;

use super::generation::PathFrame;

#[derive(Clone, Debug)]
pub struct RoadEdges {
    pub left: Vec<Vec3>,
    pub right: Vec<Vec3>,
}

pub fn road_edges(frames: &[PathFrame], width: f32) -> RoadEdges {
    let half_width = width * 0.5;
    let mut left = Vec::with_capacity(frames.len());
    let mut right = Vec::with_capacity(frames.len());

    for frame in frames {
        left.push(frame.center - frame.right * half_width);
        right.push(frame.center + frame.right * half_width);
    }

    RoadEdges { left, right }
}

pub fn line_segments(points: &[Vec3]) -> Vec<[Vec3; 2]> {
    points
        .windows(2)
        .filter_map(|pair| {
            let [start, end] = [pair[0], pair[1]];
            (start.distance_squared(end) > f32::EPSILON).then_some([start, end])
        })
        .collect()
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
            let frames = quarter_turn_frames(direction, 0.0);
            let edges = road_edges(&frames, TEST_WIDTH);

            for ((frame, left), right) in frames.iter().zip(&edges.left).zip(&edges.right) {
                assert_approx(left.distance(*right), TEST_WIDTH);
                assert_approx(((*left + *right) * 0.5).distance(frame.center), 0.0);
            }
        }
    }

    #[test]
    fn banked_road_edges_keep_width_while_changing_height() {
        let frames = quarter_turn_frames(Turn::Right, std::f32::consts::FRAC_PI_4);
        let edges = road_edges(&frames, TEST_WIDTH);

        for ((frame, left), right) in frames.iter().zip(&edges.left).zip(&edges.right) {
            assert_approx(left.distance(*right), TEST_WIDTH);
            assert_approx(((*left + *right) * 0.5).distance(frame.center), 0.0);
            assert!(right.y < left.y);
        }
    }

    #[test]
    fn turn_boundaries_do_not_cross_for_left_or_right_turns() {
        for direction in [Turn::Left, Turn::Right] {
            let frames = quarter_turn_frames(direction, 0.0);
            let edges = road_edges(&frames, TEST_WIDTH);
            let center = arc_center(direction);
            let half_width = TEST_WIDTH * 0.5;

            for (left, right) in edges.left.iter().zip(&edges.right) {
                let left_radius = Vec2::new(left.x, left.z).distance(center);
                let right_radius = Vec2::new(right.x, right.z).distance(center);

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

    fn quarter_turn_frames(direction: Turn, bank: f32) -> Vec<PathFrame> {
        let entry = Pose2::new(Vec2::ZERO, 0.0);
        let side = direction.side();
        let center = arc_center(direction);
        let radius_vector = -entry.right() * side * TEST_RADIUS;

        (0..=TEST_STEPS)
            .map(|step| {
                let t = step as f32 / TEST_STEPS as f32;
                let rotation = side * std::f32::consts::FRAC_PI_2 * t;
                let pose = Pose2::new(
                    center + rotate_2d(radius_vector, -rotation),
                    entry.yaw + rotation,
                );
                PathFrame::new(pose.position, pose.yaw, bank)
            })
            .collect()
    }

    fn arc_center(direction: Turn) -> Vec2 {
        Pose2::new(Vec2::ZERO, 0.0).right() * direction.side() * TEST_RADIUS
    }

    fn assert_approx(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.001,
            "expected {expected:.4}, got {actual:.4}"
        );
    }
}
