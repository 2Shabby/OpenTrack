use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::generation::PathFrame;
use super::path_geometry::road_edges;

pub fn road_surface_mesh(frames: &[PathFrame], width: f32) -> Mesh {
    assert!(
        frames.len() >= 2,
        "road surface mesh requires at least two frames"
    );

    let edges = road_edges(frames, width);
    let mut positions = Vec::with_capacity(frames.len() * 2);
    let mut normals = Vec::with_capacity(frames.len() * 2);
    let mut uvs = Vec::with_capacity(frames.len() * 2);
    let mut indices = Vec::with_capacity((frames.len() - 1) * 6);
    let mut distance = 0.0;

    for (index, frame) in frames.iter().enumerate() {
        if index > 0 {
            distance += frame.center.distance(frames[index - 1].center);
        }

        for point in [edges.left[index], edges.right[index]] {
            positions.push(point.to_array());
            normals.push(frame.normal.to_array());
        }
        uvs.push([0.0, distance]);
        uvs.push([1.0, distance]);
    }

    for segment in 0..frames.len() - 1 {
        let left = (segment * 2) as u32;
        let right = left + 1;
        let next_left = left + 2;
        let next_right = left + 3;
        indices.extend([left, next_left, right, right, next_left, next_right]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn road_surface_mesh_uses_oriented_frame_vertices() {
        let frames = [
            PathFrame::new(Vec2::ZERO, 0.0, 0.0),
            PathFrame::new(Vec2::new(0.0, 10.0), 0.0, 0.0),
        ];

        let mesh = road_surface_mesh(&frames, 12.0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("road mesh must have 3D positions");
        };

        assert_eq!(positions.len(), 4);
        assert!(positions.iter().all(|position| position[1].abs() < 0.001));
    }

    #[test]
    fn banked_road_surface_mesh_raises_one_edge() {
        let frames = [
            PathFrame::new(Vec2::ZERO, 0.0, std::f32::consts::FRAC_PI_4),
            PathFrame::new(Vec2::new(0.0, 10.0), 0.0, std::f32::consts::FRAC_PI_4),
        ];

        let mesh = road_surface_mesh(&frames, 12.0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("road mesh must have 3D positions");
        };

        assert!(positions[0][1] > positions[1][1]);
        assert!(positions[2][1] > positions[3][1]);
    }
}
