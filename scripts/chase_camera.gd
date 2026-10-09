extends Camera3D

const BASE_DISTANCE := 7.2
const MAX_EXTRA_DISTANCE := 1.2
const HEIGHT := 2.8
const TARGET_HEIGHT := 0.9
const LOOKAHEAD := 2.8
const BASE_FOV := 68.0
const SPEED_FOV := 6.0
const FULL_SPEED := 45.0
const HEADING_DAMPING := 6.0
const MAX_YAW_SPEED := 3.5
const HEIGHT_DAMPING := 10.0
const SPEED_DAMPING := 3.0
const MAX_SLIP_ANGLE := 0.6
const VELOCITY_BLEND := 0.25
const COLLISION_RADIUS := 0.3
const COLLISION_MARGIN := 0.15
const COLLISION_RETURN_SPEED := 5.0

var _yaw := 0.0
var _height := 0.0
var _speed_blend := 0.0
var _collision_distance := 0.0
var _has_pose := false
var _collision_query := PhysicsShapeQueryParameters3D.new()

func _ready() -> void:
	# The car supplies an interpolated render pose; interpolating the camera
	# itself would add another physics tick of chase lag.
	physics_interpolation_mode = Node.PHYSICS_INTERPOLATION_MODE_OFF
	var sphere := SphereShape3D.new()
	sphere.radius = COLLISION_RADIUS
	_collision_query.shape = sphere
	_collision_query.collision_mask = TrackGeometry.SUPPORT_MASK

func reset_follow() -> void:
	_has_pose = false

func follow(car: Node3D, velocity: Vector3, delta: float) -> void:
	var pose := car.get_global_transform_interpolated()
	var forward := (-pose.basis.z).slide(Vector3.UP)
	var planar := velocity.slide(Vector3.UP)
	var speed := planar.length()
	var desired_yaw := _yaw
	if forward.length_squared() > 0.1:
		forward = forward.normalized()
		desired_yaw = atan2(forward.x, forward.z)
		# Keep the chase behind the body in reverse and limit its orbit in a slide.
		if speed > 4.0 and planar.dot(forward) > 0.0:
			var slip := clampf(forward.signed_angle_to(planar / speed, Vector3.UP), -MAX_SLIP_ANGLE, MAX_SLIP_ANGLE)
			desired_yaw += slip * VELOCITY_BLEND * clampf((speed - 4.0) / 12.0, 0.0, 1.0)
	var desired_speed_blend := clampf(speed / FULL_SPEED, 0.0, 1.0)
	if not _has_pose:
		_yaw = desired_yaw
		_height = pose.origin.y
		_speed_blend = desired_speed_blend
	else:
		var yaw_step := wrapf(desired_yaw - _yaw, -PI, PI) * (1.0 - exp(-HEADING_DAMPING * delta))
		_yaw += clampf(yaw_step, -MAX_YAW_SPEED * delta, MAX_YAW_SPEED * delta)
		_height = lerpf(_height, pose.origin.y, 1.0 - exp(-HEIGHT_DAMPING * delta))
		_speed_blend = lerpf(_speed_blend, desired_speed_blend, 1.0 - exp(-SPEED_DAMPING * delta))
	forward = Vector3(sin(_yaw), 0, cos(_yaw))
	var anchor := Vector3(pose.origin.x, _height, pose.origin.z)
	var distance := BASE_DISTANCE + MAX_EXTRA_DISTANCE * _speed_blend
	var desired := anchor - forward * distance + Vector3.UP * HEIGHT
	var pivot := pose.origin + Vector3.UP * TARGET_HEIGHT
	var motion := desired - pivot
	_collision_query.transform = Transform3D(Basis.IDENTITY, pivot)
	_collision_query.motion = motion
	var sweep := get_world_3d().direct_space_state.cast_motion(_collision_query)
	var safe_distance := maxf(0.0, motion.length() * sweep[0] - COLLISION_MARGIN)
	if not _has_pose or safe_distance < _collision_distance:
		_collision_distance = safe_distance
	else:
		_collision_distance = lerpf(_collision_distance, safe_distance, 1.0 - exp(-COLLISION_RETURN_SPEED * delta))
	global_position = pivot + motion.normalized() * _collision_distance
	fov = BASE_FOV + SPEED_FOV * _speed_blend
	var look := anchor + Vector3.UP * TARGET_HEIGHT + forward * LOOKAHEAD
	if global_position.distance_squared_to(look) > 0.01:
		look_at(look, Vector3.UP)
	_has_pose = true
