use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;

use super::generation::PathFrame;

pub fn road_surface_mesh(frames: &[PathFrame], width: f32) -> Mesh {
    let mut positions = Vec::with_capacity(frames.len() * 2);
    let mut normals = Vec::with_capacity(frames.len() * 2);
    let mut uvs = Vec::with_capacity(frames.len() * 2);
    let mut indices = Vec::with_capacity(frames.len().saturating_sub(1) * 6);
    let mut distance = 0.0;
    let half_width = width * 0.5;

    for (index, frame) in frames.iter().copied().enumerate() {
        if index > 0 {
            distance += frames[index - 1]
                .pose
                .position
                .distance(frame.pose.position);
        }

        let right = frame.pose.right();
        let left_edge = frame.pose.position - right * half_width;
        let right_edge = frame.pose.position + right * half_width;

        positions.push([left_edge.x, 0.0, left_edge.y]);
        positions.push([right_edge.x, 0.0, right_edge.y]);
        normals.extend([[0.0, 1.0, 0.0]; 2]);
        uvs.push([0.0, distance]);
        uvs.push([1.0, distance]);
    }

    for index in 0..frames.len().saturating_sub(1) {
        let start = (index * 2) as u32;
        indices.extend([start, start + 2, start + 1, start + 1, start + 2, start + 3]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}
