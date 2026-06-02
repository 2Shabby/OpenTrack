use bevy::prelude::*;

use super::markers::{GeneratedEnvironment, GeneratedScenery, SpawnedSceneEntity};

pub fn spawn_grass_field(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(90.0, 90.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.34, 0.13),
            perceptual_roughness: 0.98,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.015, 0.0),
        GeneratedEnvironment,
        SpawnedSceneEntity,
    ));
}

pub fn spawn_forest_scenery(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let oak_leaf = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.58, 0.18),
        base_color_texture: Some(asset_server.load("forest/OakTreeLeaf.png")),
        perceptual_roughness: 0.9,
        ..default()
    });
    let oak_trunk = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.27, 0.12),
        base_color_texture: Some(asset_server.load("forest/OakTreeTrunk.png")),
        perceptual_roughness: 0.85,
        ..default()
    });
    let spruce_leaf = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.43, 0.2),
        base_color_texture: Some(asset_server.load("forest/SpruceTreeLeaf.png")),
        perceptual_roughness: 0.9,
        ..default()
    });
    let spruce_trunk = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.25, 0.11),
        base_color_texture: Some(asset_server.load("forest/SpruceTreeTrunk.png")),
        perceptual_roughness: 0.85,
        ..default()
    });
    let rock = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.43, 0.38),
        base_color_texture: Some(asset_server.load("forest/rockTexture1.png")),
        perceptual_roughness: 0.95,
        ..default()
    });

    for (index, position) in tree_positions().into_iter().enumerate() {
        if index % 2 == 0 {
            spawn_oak_tree(
                commands,
                meshes,
                oak_trunk.clone(),
                oak_leaf.clone(),
                position,
            );
        } else {
            spawn_spruce_tree(
                commands,
                meshes,
                spruce_trunk.clone(),
                spruce_leaf.clone(),
                position,
            );
        }
    }

    for (index, position) in rock_positions().into_iter().enumerate() {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.4, 0.7, 1.0))),
            MeshMaterial3d(rock.clone()),
            Transform::from_translation(position + Vec3::Y * 0.35).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                index as f32 * 0.8,
                0.15,
                0.2,
            )),
            GeneratedScenery,
            SpawnedSceneEntity,
        ));
    }
}

fn spawn_oak_tree(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    trunk_material: Handle<StandardMaterial>,
    leaf_material: Handle<StandardMaterial>,
    position: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.28, 2.4).mesh().resolution(7))),
        MeshMaterial3d(trunk_material),
        Transform::from_translation(position + Vec3::Y * 1.2),
        GeneratedScenery,
        SpawnedSceneEntity,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.45).mesh().ico(1).expect("valid ico sphere"))),
        MeshMaterial3d(leaf_material),
        Transform::from_translation(position + Vec3::Y * 2.65),
        GeneratedScenery,
        SpawnedSceneEntity,
    ));
}

fn spawn_spruce_tree(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    trunk_material: Handle<StandardMaterial>,
    leaf_material: Handle<StandardMaterial>,
    position: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.22, 2.1).mesh().resolution(7))),
        MeshMaterial3d(trunk_material),
        Transform::from_translation(position + Vec3::Y * 1.05),
        GeneratedScenery,
        SpawnedSceneEntity,
    ));

    for (height, radius, y) in [(1.8, 1.6, 2.0), (1.6, 1.25, 2.75), (1.35, 0.9, 3.35)] {
        commands.spawn((
            Mesh3d(meshes.add(Cone::new(radius, height).mesh().resolution(7))),
            MeshMaterial3d(leaf_material.clone()),
            Transform::from_translation(position + Vec3::Y * y),
            GeneratedScenery,
            SpawnedSceneEntity,
        ));
    }
}

fn tree_positions() -> [Vec3; 10] {
    [
        Vec3::new(-18.0, 0.0, -30.0),
        Vec3::new(17.0, 0.0, -28.0),
        Vec3::new(-23.0, 0.0, -18.0),
        Vec3::new(20.0, 0.0, -12.0),
        Vec3::new(-19.0, 0.0, -3.0),
        Vec3::new(24.0, 0.0, 5.0),
        Vec3::new(-22.0, 0.0, 14.0),
        Vec3::new(18.0, 0.0, 23.0),
        Vec3::new(-16.0, 0.0, 31.0),
        Vec3::new(23.0, 0.0, 33.0),
    ]
}

fn rock_positions() -> [Vec3; 4] {
    [
        Vec3::new(-13.5, 0.0, -22.0),
        Vec3::new(14.5, 0.0, -4.0),
        Vec3::new(-14.0, 0.0, 10.0),
        Vec3::new(13.5, 0.0, 28.0),
    ]
}
