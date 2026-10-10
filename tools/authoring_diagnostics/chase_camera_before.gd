extends Camera3D

const BASE_DISTANCE := 11.8
const MAX_EXTRA_DISTANCE := 4.4
const SPEED_DISTANCE_SCALE := 0.044
const HEIGHT := 5.8
const TARGET_HEIGHT := 1.2
const LOOKAHEAD := 6.8
const POSITION_DAMPING := 4.5
const TARGET_DAMPING := 2.8
const VELOCITY_BLEND_SPEED := 34.0
const MAX_VELOCITY_BLEND := 0.35

var _target := Vector3.ZERO
var _forward := Vector3(0, 0, 1)
var _has_target := false
var _has_pose := false

func reset_follow() -> void:
	_has_target = false
	_has_pose = false

func follow(car: Node3D, velocity: Vector3, delta: float) -> void:
	var target_blend := 1.0 - exp(-TARGET_DAMPING * delta)
	if not _has_target:
		_target = car.global_position
		_has_target = true
	else:
		_target = _target.lerp(car.global_position, target_blend)
	var forward := (-car.global_transform.basis.z).slide(Vector3.UP)
	if forward.length_squared() > 0.04:
		_forward = forward.normalized()
	var planar := velocity.slide(Vector3.UP)
	var speed := planar.length()
	if speed > 0.5:
		var mix := clampf(speed / VELOCITY_BLEND_SPEED, 0.0, MAX_VELOCITY_BLEND)
		_forward = _forward.lerp(planar / speed, mix).normalized()
	var distance := BASE_DISTANCE + clampf(velocity.length() * SPEED_DISTANCE_SCALE, 0.0, MAX_EXTRA_DISTANCE)
	var desired := _target - _forward * distance + Vector3.UP * HEIGHT
	var look := _target + Vector3.UP * TARGET_HEIGHT + _forward * LOOKAHEAD
	if not _has_pose:
		global_position = desired
		_has_pose = true
	else:
		global_position = global_position.lerp(desired, 1.0 - exp(-POSITION_DAMPING * delta))
	if absf(_forward.dot(Vector3.UP)) > 0.98:
		return
	look_at(look, Vector3.UP)
