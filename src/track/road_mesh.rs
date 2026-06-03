use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy_procedural_meshes::PMesh;

use super::generation::PathFrame;
use super::path_geometry::{polygon_points, road_polygon};

pub fn road_surface_mesh(frames: &[PathFrame], width: f32) -> Mesh {
    let mut mesh = PMesh::<u32>::new();
    let polygon = road_polygon(frames, width);

    mesh.fill(0.01, |builder| {
        let points = polygon_points(&polygon);
        let Some(first) = points.first().copied() else {
            return;
        };

        builder.begin(first);
        for point in points.into_iter().skip(1) {
            builder.line_to(point);
        }
        builder.close();
    });

    mesh.flip_yz().to_bevy(RenderAssetUsages::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    use crate::geometry::Pose2;

    #[test]
    fn road_surface_mesh_uses_ground_plane_vertices() {
        let frames = [
            PathFrame {
                pose: Pose2::new(Vec2::ZERO, 0.0),
            },
            PathFrame {
                pose: Pose2::new(Vec2::new(0.0, 10.0), 0.0),
            },
        ];

        let mesh = road_surface_mesh(&frames, 12.0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("road mesh must have 3D positions");
        };

        assert!(positions.len() >= 4);
        assert!(positions.iter().all(|position| position[1].abs() < 0.001));
    }
}
