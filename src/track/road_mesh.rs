use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy_procedural_meshes::PMesh;

use super::generation::PathFrame;

pub fn road_surface_mesh(frames: &[PathFrame], width: f32) -> Mesh {
    let mut mesh = PMesh::<u32>::new();
    let half_width = width * 0.5;

    mesh.fill(0.01, |builder| {
        let mut left_edges = Vec::with_capacity(frames.len());
        let mut right_edges = Vec::with_capacity(frames.len());

        for frame in frames {
            let right = frame.pose.right();
            left_edges.push(frame.pose.position - right * half_width);
            right_edges.push(frame.pose.position + right * half_width);
        }

        if let Some(first_left) = left_edges.first().copied() {
            builder.begin(first_left);
            for right_edge in right_edges {
                builder.line_to(right_edge);
            }
            for left_edge in left_edges.into_iter().rev() {
                builder.line_to(left_edge);
            }
            builder.close();
        }
    });

    mesh.flip_yz().to_bevy(RenderAssetUsages::default())
}
