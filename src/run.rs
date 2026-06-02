use bevy::prelude::*;

use crate::driving::PlayerCar;
use crate::game_state::{GameState, not_paused};
use crate::spatial::{OrientedRect, Pose2};

pub struct RunPlugin;

impl Plugin for RunPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RunState::new(1)).add_systems(
            FixedUpdate,
            update_run.run_if(in_state(GameState::Driving).and(not_paused)),
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    Waiting,
    Running,
    Finished,
}

#[derive(Resource, Debug)]
pub struct RunState {
    pub status: RunStatus,
    pub elapsed: f32,
    pub next_checkpoint: usize,
    pub checkpoint_count: usize,
    pub checkpoint_splits: Vec<f32>,
    pub finish_recorded: bool,
}

impl RunState {
    pub fn new(checkpoint_count: usize) -> Self {
        Self {
            status: RunStatus::Waiting,
            elapsed: 0.0,
            next_checkpoint: 0,
            checkpoint_count,
            checkpoint_splits: Vec::with_capacity(checkpoint_count),
            finish_recorded: false,
        }
    }

    pub fn reset(&mut self) {
        self.status = RunStatus::Waiting;
        self.elapsed = 0.0;
        self.next_checkpoint = 0;
        self.checkpoint_splits.clear();
        self.finish_recorded = false;
    }

    pub fn status_label(&self) -> &'static str {
        match self.status {
            RunStatus::Waiting => "waiting",
            RunStatus::Running => "running",
            RunStatus::Finished => "finished",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackTriggerKind {
    Checkpoint(usize),
    Finish,
}

#[derive(Component)]
pub struct TrackTrigger {
    pub kind: TrackTriggerKind,
    pub bounds: OrientedRect,
}

impl TrackTrigger {
    pub fn new(kind: TrackTriggerKind, pose: Pose2, half_extents: Vec2) -> Self {
        Self {
            kind,
            bounds: OrientedRect::new(pose, half_extents),
        }
    }

    pub fn contains(&self, position: Vec3) -> bool {
        self.bounds.contains_xz(position)
    }
}

fn update_run(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    car: Single<(&Transform, &PlayerCar)>,
    triggers: Query<&TrackTrigger>,
    mut run: ResMut<RunState>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        run.reset();
        return;
    }

    let (transform, car) = *car;

    if run.status == RunStatus::Waiting && (car.throttle.abs() > 0.0 || car.velocity.length() > 0.2)
    {
        run.status = RunStatus::Running;
    }

    if run.status != RunStatus::Running {
        return;
    }

    run.elapsed += time.delta_secs();
    let elapsed = run.elapsed;

    for trigger in &triggers {
        if !trigger.contains(transform.translation) {
            continue;
        }

        match trigger.kind {
            TrackTriggerKind::Checkpoint(index) if index == run.next_checkpoint => {
                run.checkpoint_splits.push(elapsed);
                run.next_checkpoint += 1;
            }
            TrackTriggerKind::Finish if run.next_checkpoint >= run.checkpoint_count => {
                run.status = RunStatus::Finished;
            }
            _ => {}
        }
    }
}
