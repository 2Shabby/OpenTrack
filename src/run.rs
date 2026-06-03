use bevy::prelude::*;

use crate::driving::PlayerCar;
use crate::game_state::{GameState, not_paused};
use crate::geometry::OrientedRect;

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
    pub fn crosses(&self, start: Vec3, end: Vec3) -> bool {
        self.bounds.contains_xz(start)
            || self.bounds.contains_xz(end)
            || segment_intersects_bounds(
                self.bounds
                    .pose
                    .world_to_local(crate::geometry::xz_position(start)),
                self.bounds
                    .pose
                    .world_to_local(crate::geometry::xz_position(end)),
                self.bounds.half_extents,
            )
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
        if !trigger.crosses(car.previous_translation, transform.translation) {
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

fn segment_intersects_bounds(start: Vec2, end: Vec2, half_extents: Vec2) -> bool {
    let delta = end - start;
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;

    for (position, velocity, extent) in [
        (start.x, delta.x, half_extents.x),
        (start.y, delta.y, half_extents.y),
    ] {
        if velocity.abs() <= f32::EPSILON {
            if position.abs() > extent {
                return false;
            }
            continue;
        }

        let near = (-extent - position) / velocity;
        let far = (extent - position) / velocity;
        enter = enter.max(near.min(far));
        exit = exit.min(near.max(far));

        if enter > exit {
            return false;
        }
    }

    exit >= 0.0 && enter <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::geometry::{Pose2, xz_translation};

    #[test]
    fn trigger_detects_fast_crossing_without_endpoint_inside() {
        let trigger = TrackTrigger {
            kind: TrackTriggerKind::Finish,
            bounds: OrientedRect::new(Pose2::new(Vec2::ZERO, 0.0), Vec2::new(6.0, 0.45)),
        };

        assert!(trigger.crosses(
            xz_translation(Vec2::new(0.0, -4.0), 0.0),
            xz_translation(Vec2::new(0.0, 4.0), 0.0)
        ));
    }

    #[test]
    fn trigger_ignores_parallel_miss() {
        let trigger = TrackTrigger {
            kind: TrackTriggerKind::Finish,
            bounds: OrientedRect::new(Pose2::new(Vec2::ZERO, 0.0), Vec2::new(6.0, 0.45)),
        };

        assert!(!trigger.crosses(
            xz_translation(Vec2::new(7.0, -4.0), 0.0),
            xz_translation(Vec2::new(7.0, 4.0), 0.0)
        ));
    }
}
