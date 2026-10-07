class_name RallyCar
extends Vehicle

const BODY_FORWARD := Vector3(0, 0, -1)
const SPAWN_MARGIN := 0.04

var accept_input := true
var suppress_until_release := false
var clearance := 0.2
var _prepared := false
var _waiting_for_drive := true

func place_at(road_pose: Transform3D) -> void:
	prepare()
	var up := road_pose.basis.y
	# Turn +Z stage-forward into -Z vehicle-forward with a rotation, not a reflection.
	transform = Transform3D(Basis(-road_pose.basis.x, up, -road_pose.basis.z), road_pose.origin + up * clearance)

func prepare() -> void:
	if _prepared:
		return
	_apply_surfaces()
	collision_layer = TrackGeometry.CAR_LAYER
	collision_mask = TrackGeometry.SUPPORT_MASK
	continuous_cd = true
	var visual := get_node("Visual") as Node3D
	var wheels: Dictionary = visual.bind_wheels()
	_mount(front_left_wheel, wheels["front_left"], true)
	_mount(front_right_wheel, wheels["front_right"], true)
	_mount(rear_left_wheel, wheels["rear_left"], false)
	_mount(rear_right_wheel, wheels["rear_right"], false)
	front_tire_radius = wheels["front_left"]["radius"]
	rear_tire_radius = wheels["rear_left"]["radius"]
	front_tire_width = wheels["front_left"]["width"] * 1000.0
	rear_tire_width = wheels["rear_left"]["width"] * 1000.0
	_fit_chassis()
	var contact := front_left_wheel.position.y - front_spring_length - front_tire_radius
	clearance = -contact + SPAWN_MARGIN
	_prepared = true

func _ready() -> void:
	prepare()
	super._ready()

func _physics_process(delta: float) -> void:
	if freeze:
		_zero_inputs()
		return
	if accept_input:
		_read_input()
	super._physics_process(delta)

func clear_held_input() -> void:
	_zero_inputs()
	suppress_until_release = true

func forward_vector() -> Vector3:
	return global_basis * BODY_FORWARD

func contact_count() -> int:
	return get_wheel_contact_count() if is_ready else 0

func airborne() -> bool:
	return contact_count() == 0

func telemetry() -> Dictionary:
	var names: PackedStringArray = []
	var counts := {}
	if is_ready:
		for wheel in wheel_array:
			var label := ""
			if wheel.is_colliding():
				label = str(wheel.surface_type)
				counts[label] = int(counts.get(label, 0)) + 1
			names.append(label)
	var surface := "air"
	var best := 0
	for key in counts:
		if int(counts[key]) >= best:
			best = int(counts[key])
			surface = _display_surface(str(key))
	return {
		"speed": linear_velocity.length(),
		"signed_speed": linear_velocity.dot(forward_vector()),
		"velocity": linear_velocity,
		"angular_velocity": angular_velocity,
		"surface": surface,
		"contacts": contact_count(),
		"support": "air" if contact_count() == 0 else ("grounded" if contact_count() >= 3 else "partial"),
		"gear": current_gear,
		"rpm": motor_rpm,
		"steer": steering_amount,
		"wheels": names,
	}

func freeze_at_finish() -> void:
	accept_input = false
	_waiting_for_drive = false
	_zero_inputs()
	linear_velocity = Vector3.ZERO
	angular_velocity = Vector3.ZERO
	freeze = true
	for wheel: Wheel in wheel_array:
		wheel.set_process(false)

func _apply_surfaces() -> void:
	var friction := {}
	var stiffness := {}
	var rolling := {}
	var lateral := {}
	var longitudinal := {}
	for profile in TrackGeometry.SURFACES:
		var key := StringName(profile.group_name)
		friction[key] = profile.friction
		stiffness[key] = profile.stiffness
		rolling[key] = profile.rolling_resistance
		lateral[key] = profile.lateral_grip_assist
		longitudinal[key] = profile.longitudinal_grip_ratio
	coefficient_of_friction = friction
	tire_stiffnesses = stiffness
	rolling_resistance = rolling
	lateral_grip_assist = lateral
	longitudinal_grip_ratio = longitudinal

func _mount(ray: Wheel, spec: Dictionary, front: bool) -> void:
	var rest := (front_spring_length if front else rear_spring_length) * (front_resting_ratio if front else rear_resting_ratio)
	var center: Vector3 = spec["center"]
	ray.position = center + Vector3.UP * rest
	ray.collision_mask = TrackGeometry.SUPPORT_MASK
	ray.exclude_parent = true
	ray.enabled = true
	ray.add_exception(self)
	var pivot := Node3D.new()
	pivot.name = "Visual"
	ray.add_child(pivot)
	var mesh_instance := MeshInstance3D.new()
	mesh_instance.mesh = spec["mesh"]
	pivot.add_child(mesh_instance)
	ray.wheel_node = pivot

func _fit_chassis() -> void:
	var visual := get_node("Visual") as Node3D
	var chassis_shape := get_node("CollisionShape3D") as CollisionShape3D
	var body: MeshInstance3D = visual.get_node("Model/SportsCar_Body")
	var to_car: Transform3D = visual.to_car_space(body)
	var aabb: AABB = body.mesh.get_aabb()
	var minimum := Vector3(INF, INF, INF)
	var maximum := Vector3(-INF, -INF, -INF)
	for corner in 8:
		var point := to_car * aabb.get_endpoint(corner)
		minimum = minimum.min(point)
		maximum = maximum.max(point)
	var shape := BoxShape3D.new()
	shape.size = maximum - minimum
	shape.margin = 0.01
	chassis_shape.shape = shape
	chassis_shape.position = (minimum + maximum) * 0.5

func _read_input() -> void:
	var throttle := Input.get_action_strength("throttle_positive")
	var brake := Input.get_action_strength("throttle_negative")
	var left := Input.get_action_strength("steer_left")
	var right := Input.get_action_strength("steer_right")
	var steer := left - right
	var handbrake := Input.get_action_strength("rear_brake")
	if suppress_until_release:
		if throttle < 0.01 and brake < 0.01 and left < 0.01 and right < 0.01 and handbrake < 0.01:
			suppress_until_release = false
		else:
			_zero_inputs()
			return
	if current_gear == -1:
		throttle_input = brake
		brake_input = throttle
	else:
		throttle_input = throttle
		brake_input = brake
	steering_input = steer
	if throttle > 0.01 or brake > 0.01:
		_waiting_for_drive = false
	handbrake_input = 1.0 if _waiting_for_drive else handbrake
	clutch_input = handbrake_input

func drive_requested() -> bool:
	return not freeze and not suppress_until_release and (throttle_input > 0.01 or brake_input > 0.01)

func _zero_inputs() -> void:
	throttle_input = 0.0
	brake_input = 0.0
	steering_input = 0.0
	handbrake_input = 1.0 if _waiting_for_drive else 0.0
	clutch_input = handbrake_input

func _display_surface(group: String) -> String:
	for profile in TrackGeometry.SURFACES:
		if profile.group_name == group:
			return profile.display_name
	return group.to_lower()
