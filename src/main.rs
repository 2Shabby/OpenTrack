use bevy::prelude::*;

const CAR_START: Vec3 = Vec3::new(0.0, 0.35, -14.0);

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.055)))
        .insert_resource(DrivingTuning::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Open Track Turbo - Driving Sandbox".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup_sandbox)
        .add_systems(Update, (drive_car, chase_camera.after(drive_car)))
        .run();
}

#[derive(Resource)]
struct DrivingTuning {
    acceleration: f32,
    brake_force: f32,
    reverse_force: f32,
    steer_rate: f32,
    lateral_grip: f32,
    drag: f32,
    max_forward_speed: f32,
    max_reverse_speed: f32,
}

impl Default for DrivingTuning {
    fn default() -> Self {
        Self {
            acceleration: 38.0,
            brake_force: 52.0,
            reverse_force: 24.0,
            steer_rate: 2.5,
            lateral_grip: 8.5,
            drag: 0.9,
            max_forward_speed: 48.0,
            max_reverse_speed: 14.0,
        }
    }
}

#[derive(Component)]
struct PlayerCar {
    velocity: Vec3,
    yaw: f32,
}

#[derive(Component)]
struct ChaseCamera;

fn setup_sandbox(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(80.0, 80.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.22, 0.19),
            perceptual_roughness: 0.95,
            ..default()
        })),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.35, 0.55, 2.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.13, 0.08),
            ..default()
        })),
        Transform::from_translation(CAR_START),
        PlayerCar {
            velocity: Vec3::ZERO,
            yaw: 0.0,
        },
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 7_500.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.8, 0.0)),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 6.5, -22.0).looking_at(CAR_START, Vec3::Y),
        ChaseCamera,
    ));
}

fn drive_car(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    tuning: Res<DrivingTuning>,
    mut cars: Query<(&mut Transform, &mut PlayerCar)>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut car) in &mut cars {
        if keys.just_pressed(KeyCode::KeyR) {
            transform.translation = CAR_START;
            car.velocity = Vec3::ZERO;
            car.yaw = 0.0;
        }

        let throttle = axis(
            keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]),
            keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]),
        );
        let steer = axis(
            keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]),
            keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]),
        );

        let forward = Vec3::new(car.yaw.sin(), 0.0, car.yaw.cos());
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        let forward_speed = car.velocity.dot(forward);
        let lateral_speed = car.velocity.dot(right);

        let speed_ratio = (forward_speed.abs() / tuning.max_forward_speed).clamp(0.0, 1.0);
        let steer_authority = 0.35 + speed_ratio * 0.65;
        car.yaw -= steer * tuning.steer_rate * steer_authority * dt;

        let drive_force = if throttle >= 0.0 {
            tuning.acceleration
        } else if forward_speed > 1.0 {
            tuning.brake_force
        } else {
            tuning.reverse_force
        };

        car.velocity += forward * throttle * drive_force * dt;
        car.velocity -= right * lateral_speed * tuning.lateral_grip * dt;
        car.velocity *= 1.0 / (1.0 + tuning.drag * dt);

        let capped_forward_speed = car
            .velocity
            .dot(forward)
            .clamp(-tuning.max_reverse_speed, tuning.max_forward_speed);
        let capped_lateral_speed = car.velocity.dot(right);
        car.velocity = forward * capped_forward_speed + right * capped_lateral_speed;

        transform.translation += car.velocity * dt;
        transform.translation.y = CAR_START.y;
        transform.rotation = Quat::from_rotation_y(car.yaw);
    }
}

fn chase_camera(
    time: Res<Time>,
    car: Single<(&Transform, &PlayerCar), With<PlayerCar>>,
    mut camera: Single<&mut Transform, (With<ChaseCamera>, Without<PlayerCar>)>,
) {
    let (car_transform, car_state) = *car;
    let speed = car_state.velocity.length();
    let forward = Vec3::new(car_state.yaw.sin(), 0.0, car_state.yaw.cos());
    let target = car_transform.translation + Vec3::Y * 1.0;
    let desired_position = target - forward * (7.5 + speed * 0.06) + Vec3::Y * 4.2;
    let smoothing = 1.0 - (-8.0 * time.delta_secs()).exp();

    camera.translation = camera.translation.lerp(desired_position, smoothing);
    camera.look_at(target + forward * 4.0, Vec3::Y);
}

fn axis(positive: bool, negative: bool) -> f32 {
    match (positive, negative) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}
