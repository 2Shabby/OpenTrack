use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Pose2 {
    pub position: Vec2,
    pub yaw: f32,
}

impl Pose2 {
    pub const fn new(position: Vec2, yaw: f32) -> Self {
        Self { position, yaw }
    }

    pub fn forward(self) -> Vec2 {
        Vec2::new(self.yaw.sin(), self.yaw.cos())
    }

    pub fn right(self) -> Vec2 {
        Vec2::new(self.yaw.cos(), -self.yaw.sin())
    }

    pub fn local_to_world(self, local: Vec2) -> Vec2 {
        self.position + rotate_2d(local, self.yaw)
    }

    pub fn world_to_local(self, world: Vec2) -> Vec2 {
        rotate_2d(world - self.position, -self.yaw)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OrientedRect {
    pub pose: Pose2,
    pub half_extents: Vec2,
}

impl OrientedRect {
    pub const fn new(pose: Pose2, half_extents: Vec2) -> Self {
        Self { pose, half_extents }
    }

    pub fn contains_xz(self, position: Vec3) -> bool {
        self.contains(Vec2::new(position.x, position.z))
    }

    pub fn contains(self, position: Vec2) -> bool {
        let local = self.pose.world_to_local(position);

        local.x.abs() <= self.half_extents.x && local.y.abs() <= self.half_extents.y
    }

    pub fn intersects(self, other: Self) -> bool {
        let axes = [
            self.pose.right(),
            self.pose.forward(),
            other.pose.right(),
            other.pose.forward(),
        ];

        axes.into_iter()
            .all(|axis| projections_overlap(self, other, axis.normalize_or_zero()))
    }

    pub fn shrunken(self, amount: f32) -> Option<Self> {
        let half_extents = self.half_extents - Vec2::splat(amount);
        (half_extents.x > 0.0 && half_extents.y > 0.0).then(|| Self::new(self.pose, half_extents))
    }
}

pub fn forward_3d(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

pub fn rotate_2d(value: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(value.x * cos - value.y * sin, value.x * sin + value.y * cos)
}

fn projections_overlap(a: OrientedRect, b: OrientedRect, axis: Vec2) -> bool {
    if axis == Vec2::ZERO {
        return true;
    }

    let distance = (b.pose.position - a.pose.position).dot(axis).abs();
    let a_radius = projected_radius(a, axis);
    let b_radius = projected_radius(b, axis);

    distance <= a_radius + b_radius
}

fn projected_radius(rect: OrientedRect, axis: Vec2) -> f32 {
    rect.half_extents.x * rect.pose.right().dot(axis).abs()
        + rect.half_extents.y * rect.pose.forward().dot(axis).abs()
}
