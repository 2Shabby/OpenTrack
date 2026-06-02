use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;

const SPORTS_CAR_OBJ: &str = include_str!("../assets/cars/SportsCar.obj");

pub fn sports_car_mesh() -> Mesh {
    obj_to_mesh(SPORTS_CAR_OBJ)
}

fn obj_to_mesh(source: &str) -> Mesh {
    let mut source_positions = Vec::new();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for line in source.lines() {
        let mut parts = line.split_whitespace();
        let Some(kind) = parts.next() else {
            continue;
        };

        match kind {
            "v" => {
                let x = parse_f32(parts.next());
                let y = parse_f32(parts.next());
                let z = parse_f32(parts.next());
                source_positions.push(Vec3::new(x, y, z));
            }
            "f" => {
                let face = parts
                    .filter_map(parse_obj_position_index)
                    .filter_map(|index| source_positions.get(index).copied())
                    .collect::<Vec<_>>();

                for i in 1..face.len().saturating_sub(1) {
                    let triangle = [face[0], face[i], face[i + 1]];
                    let normal = (triangle[1] - triangle[0])
                        .cross(triangle[2] - triangle[0])
                        .normalize_or_zero();

                    for vertex in triangle {
                        indices.push(positions.len() as u32);
                        positions.push(vertex.to_array());
                        normals.push(normal.to_array());
                        uvs.push([0.0, 0.0]);
                    }
                }
            }
            _ => {}
        }
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

fn parse_f32(value: Option<&str>) -> f32 {
    value.and_then(|value| value.parse().ok()).unwrap_or(0.0)
}

fn parse_obj_position_index(value: &str) -> Option<usize> {
    value
        .split('/')
        .next()
        .and_then(|index| index.parse::<usize>().ok())
        .and_then(|index| index.checked_sub(1))
}
