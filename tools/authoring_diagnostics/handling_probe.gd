extends SceneTree

var studio: Node3D
var floor_body: StaticBody3D
var car: RallyCar

func _initialize() -> void:
	_run.call_deferred()

func _frames(count: int) -> void:
	for _i in count:
		await physics_frame

func _spawn(surface: String) -> void:
	if is_instance_valid(car):
		car.free()
	for group in floor_body.get_groups():
		floor_body.remove_from_group(group)
	floor_body.add_to_group(surface)
	car = root.get_node("Game").car_scene.instantiate()
	car.accept_input = false
	car.configure(19)
	assert(car.place_at(Transform3D.IDENTITY), car.configuration_error)
	studio.add_child(car)
	car.handbrake_input = 1.0
	car.clutch_input = 1.0
	await _frames(240)
	car.handbrake_input = 0.0
	car.clutch_input = 0.0

func _run() -> void:
	studio = Node3D.new()
	root.add_child(studio)
	floor_body = StaticBody3D.new()
	floor_body.collision_layer = TrackGeometry.SUPPORT_MASK
	floor_body.position.y = -0.5
	studio.add_child(floor_body)
	var collider := CollisionShape3D.new()
	collider.shape = BoxShape3D.new()
	collider.shape.size = Vector3(200, 1, 8000)
	floor_body.add_child(collider)
	var report := {}
	for surface in ["Road", "Dirt", "Grass"]:
		await _spawn(surface)
		car.throttle_input = 1.0
		await _frames(720)
		var acceleration_6 := car.linear_velocity.dot(car.forward_vector())
		await _frames(1680)
		var acceleration_20 := car.linear_velocity.dot(car.forward_vector())
		await _spawn(surface)
		car.linear_velocity = car.forward_vector() * 25.0
		car.clutch_input = 1.0
		for wheel: Wheel in car.wheel_array:
			wheel.spin = 25.0 / wheel.tire_radius
		await _frames(120)
		var coast_1 := car.linear_velocity.dot(car.forward_vector())
		await _frames(240)
		var coast_3 := car.linear_velocity.dot(car.forward_vector())
		car.brake_input = 1.0
		var brake_seconds := 0.0
		while car.linear_velocity.length() > 1.0 and brake_seconds < 10.0:
			await physics_frame
			brake_seconds += 1.0 / 120.0
		report[surface] = {"drive_6_mps": acceleration_6, "drive_20_mps": acceleration_20, "coast_from_25_after_1_mps": coast_1, "coast_after_3_mps": coast_3, "brake_to_1_seconds": brake_seconds, "telemetry": car.telemetry()}
	print("HANDLING_PROBE ", JSON.stringify(report))
	studio.free()
	quit()
