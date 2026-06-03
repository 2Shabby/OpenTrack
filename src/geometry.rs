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

    pub fn contains(self, position: Vec2) -> bool {
        let local = self.pose.world_to_local(position);

        local.x.abs() <= self.half_extents.x && local.y.abs() <= self.half_extents.y
    }

    pub fn corners(self) -> [Vec2; 4] {
        let right = self.pose.right() * self.half_extents.x;
        let forward = self.pose.forward() * self.half_extents.y;

        [
            self.pose.position - right - forward,
            self.pose.position + right - forward,
            self.pose.position + right + forward,
            self.pose.position - right + forward,
        ]
    }
}

pub fn forward_3d(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

pub fn right_3d(yaw: f32) -> Vec3 {
    let forward = forward_3d(yaw);
    Vec3::new(forward.z, 0.0, -forward.x)
}

pub fn xz_translation(position: Vec2, y: f32) -> Vec3 {
    Vec3::new(position.x, y, position.y)
}

pub fn rotation_from_yaw_and_up(yaw: f32, up: Vec3) -> Quat {
    let up = up.normalize_or(Vec3::Y);
    let flat_forward = forward_3d(yaw);
    let forward = (flat_forward - up * flat_forward.dot(up)).normalize_or(flat_forward);
    let right = up.cross(forward).normalize_or(right_3d(yaw));
    let forward = right.cross(up).normalize_or(forward);

    Quat::from_mat3(&Mat3::from_cols(right, up, forward))
}

pub fn rotate_2d(value: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(value.x * cos - value.y * sin, value.x * sin + value.y * cos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_up_rotation_aligns_local_up_to_surface_normal() {
        let normal = Vec3::new(-0.29, 0.68, 0.68).normalize();
        let rotation = rotation_from_yaw_and_up(1.2, normal);

        assert!((rotation * Vec3::Y).distance(normal) < 0.001);
        assert!((rotation * Vec3::Z).dot(normal).abs() < 0.001);
    }
}
