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

    pub fn local_to_world(self, local: Vec2) -> Vec2 {
        self.position + rotate_2d(local, self.yaw)
    }

    pub fn world_to_local(self, world: Vec2) -> Vec2 {
        rotate_2d(world - self.position, -self.yaw)
    }

    pub fn transform(self) -> Transform {
        Transform::from_xyz(self.position.x, 0.0, self.position.y)
            .with_rotation(Quat::from_rotation_y(self.yaw))
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

    pub fn overlap_point(self, position: Vec2, padding: Vec2) -> Option<Overlap2> {
        let local = self.pose.world_to_local(position);
        let expanded = self.half_extents + padding;

        if local.x.abs() > expanded.x || local.y.abs() > expanded.y {
            return None;
        }

        let penetration_x = expanded.x - local.x.abs();
        let penetration_y = expanded.y - local.y.abs();
        let local_normal = if penetration_x <= penetration_y {
            Vec2::new(signum_or_one(local.x), 0.0)
        } else {
            Vec2::new(0.0, signum_or_one(local.y))
        };

        Some(Overlap2 {
            normal: rotate_2d(local_normal, self.pose.yaw),
            penetration: penetration_x.min(penetration_y),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Overlap2 {
    pub normal: Vec2,
    pub penetration: f32,
}

pub fn forward_3d(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

pub fn rotate_2d(value: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(value.x * cos - value.y * sin, value.x * sin + value.y * cos)
}

fn signum_or_one(value: f32) -> f32 {
    if value >= 0.0 { 1.0 } else { -1.0 }
}
