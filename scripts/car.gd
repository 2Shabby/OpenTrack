class_name RallyCar
extends Vehicle

const BODY_FORWARD := Vector3(0, 0, -1)
const SPAWN_MARGIN := 0.04

var accept_input := true
var suppress_until_release := false
var clearance := 0.2
var _prepared := false
var _waiting_for_drive := true
var configuration_error := ""
var _paint_index := 19
var visual: CarVisual

func configure(palette_index: int) -> void:
	assert(palette_index >= 0 and palette_index < 32)
	_paint_index = palette_index
	if _prepared:
		visual.configure(palette_index)

func place_at(road_pose: Transform3D) -> bool:
	if not prepare():
		return false
	var up := road_pose.basis.y
	# Turn +Z stage-forward into -Z vehicle-forward with a rotation, not a reflection.
	transform = Transform3D(Basis(-road_pose.basis.x, up, -road_pose.basis.z), road_pose.origin + up * clearance)
	return true

func prepare() -> bool:
	if _prepared:
		return true
	visual = get_node_or_null("Visual") as CarVisual
	configuration_error = "Car scene requires a CarVisual child named Visual." if visual == null else visual.validation_error()
	if not configuration_error.is_empty():
		return false
	var rays := suspension_wheel_nodes()
	if rays.has(null) or rays.size() != visual.wheels.size() or (front_center_wheel != null and (front_left_wheel != null or front_right_wheel != null)):
		configuration_error = "Car scene requires matching distinct three- or four-wheel visual and suspension bindings."
		return false
	var unique_rays := {}
	for ray in rays:
		if unique_rays.has(ray) or ray.get_parent() != self:
			configuration_error = "Suspension bindings must be distinct wheels directly under the rigid body."
			return false
		unique_rays[ray] = true
	var chassis_shapes := 0
	for child in get_children():
		if child is CollisionShape3D and not child.disabled:
			if not (child.shape is BoxShape3D or child.shape is SphereShape3D or child.shape is CapsuleShape3D or child.shape is CylinderShape3D or child.shape is ConvexPolygonShape3D):
				configuration_error = "Car chassis collision requires primitive or convex shapes."
				return false
			chassis_shapes += 1
	if chassis_shapes == 0:
		configuration_error = "Car scene requires chassis collision shapes on its rigid body."
		return false
	visual.configure(_paint_index)
	_apply_surfaces()
	collision_layer = TrackGeometry.CAR_LAYER
	collision_mask = TrackGeometry.SUPPORT_MASK
	continuous_cd = true
	var wheels := visual.wheel_bindings()
	var front_count := front_wheel_nodes().size()
	for i in rays.size():
		_mount(rays[i], wheels[i], i < front_count)
	front_tire_radius = wheels[0]["radius"]
	rear_tire_radius = wheels[front_count]["radius"]
	front_tire_width = wheels[0]["width"] * 1000.0
	rear_tire_width = wheels[front_count]["width"] * 1000.0
	clearance = SPAWN_MARGIN
	for i in rays.size():
		var length := front_spring_length if i < front_count else rear_spring_length
		clearance = maxf(clearance, -rays[i].position.y + length + wheels[i]["radius"] + SPAWN_MARGIN)
	_prepared = true
	return true

func _ready() -> void:
	if not prepare():
		push_error(configuration_error)
		set_physics_process(false)
		return
	super._ready()

func _physics_process(delta: float) -> void:
	if freeze:
		_zero_inputs()
		return
	if accept_input:
		_read_input()
	super._physics_process(delta)
	visual.set_lights(brake_amount, current_gear == -1)

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
	var pivot: Node3D = spec["pivot"]
	pivot.owner = null
	pivot.get_parent().remove_child(pivot)
	ray.add_child(pivot)
	pivot.transform = Transform3D.IDENTITY
	ray.wheel_node = pivot

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
