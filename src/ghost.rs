use bevy::prelude::*;

use crate::car_asset::sports_car_mesh;
use crate::driving::PlayerCar;
use crate::run::{RunState, RunStatus};

const GHOST_SAMPLE_INTERVAL: f32 = 1.0 / 20.0;

pub struct GhostPlugin;

impl Plugin for GhostPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GhostRecorder::default())
            .insert_resource(SessionBestGhost::default())
            .add_systems(Startup, spawn_ghost_visual)
            .add_systems(
                Update,
                (
                    reset_recorder_when_waiting,
                    record_ghost_samples,
                    save_finished_ghost,
                    update_ghost_visual,
                ),
            );
    }
}

#[derive(Clone, Copy, Debug)]
struct GhostSample {
    time: f32,
    translation: Vec3,
    yaw: f32,
}

#[derive(Clone, Debug)]
struct GhostReplay {
    finish_time: f32,
    samples: Vec<GhostSample>,
}

#[derive(Resource, Default)]
struct GhostRecorder {
    samples: Vec<GhostSample>,
    accumulator: f32,
    saved_finished_run: bool,
}

#[derive(Resource, Default)]
pub struct SessionBestGhost {
    replay: Option<GhostReplay>,
}

impl SessionBestGhost {
    pub fn finish_time(&self) -> Option<f32> {
        self.replay.as_ref().map(|replay| replay.finish_time)
    }
}

#[derive(Component)]
struct GhostVisual;

fn spawn_ghost_visual(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(sports_car_mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.25, 0.75, 1.0, 0.38),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, -10.0, 0.0),
        Visibility::Hidden,
        GhostVisual,
    ));
}

fn reset_recorder_when_waiting(run: Res<RunState>, mut recorder: ResMut<GhostRecorder>) {
    if run.status != RunStatus::Waiting || run.elapsed > 0.0 {
        return;
    }

    recorder.samples.clear();
    recorder.accumulator = 0.0;
    recorder.saved_finished_run = false;
}

fn record_ghost_samples(
    time: Res<Time>,
    run: Res<RunState>,
    car: Single<&Transform, With<PlayerCar>>,
    mut recorder: ResMut<GhostRecorder>,
) {
    if run.status != RunStatus::Running {
        return;
    }

    recorder.accumulator += time.delta_secs();
    if recorder.accumulator < GHOST_SAMPLE_INTERVAL {
        return;
    }

    recorder.accumulator = 0.0;
    recorder.samples.push(GhostSample {
        time: run.elapsed,
        translation: car.translation,
        yaw: car.rotation.to_euler(EulerRot::YXZ).0,
    });
}

fn save_finished_ghost(
    run: Res<RunState>,
    mut recorder: ResMut<GhostRecorder>,
    mut best: ResMut<SessionBestGhost>,
) {
    if run.status != RunStatus::Finished
        || recorder.saved_finished_run
        || recorder.samples.is_empty()
    {
        return;
    }

    let should_replace = best
        .replay
        .as_ref()
        .map(|replay| run.elapsed < replay.finish_time)
        .unwrap_or(true);

    if should_replace {
        best.replay = Some(GhostReplay {
            finish_time: run.elapsed,
            samples: recorder.samples.clone(),
        });
    }

    recorder.saved_finished_run = true;
}

fn update_ghost_visual(
    run: Res<RunState>,
    best: Res<SessionBestGhost>,
    mut ghost: Single<(&mut Transform, &mut Visibility), With<GhostVisual>>,
) {
    let Some(replay) = &best.replay else {
        ghost.1.set_if_neq(Visibility::Hidden);
        return;
    };

    if run.status != RunStatus::Running {
        ghost.1.set_if_neq(Visibility::Hidden);
        return;
    }

    let Some(sample) = sample_at(replay, run.elapsed) else {
        ghost.1.set_if_neq(Visibility::Hidden);
        return;
    };

    ghost.0.translation = sample.translation + Vec3::Y * 0.08;
    ghost.0.rotation = Quat::from_rotation_y(sample.yaw);
    ghost.1.set_if_neq(Visibility::Visible);
}

fn sample_at(replay: &GhostReplay, elapsed: f32) -> Option<GhostSample> {
    replay
        .samples
        .iter()
        .copied()
        .take_while(|sample| sample.time <= elapsed)
        .last()
}
