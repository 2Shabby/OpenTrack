extends SceneTree

var failures := 0

func _initialize() -> void:
	_run.call_deferred()

func check(condition: bool, label: String) -> void:
	if not condition:
		failures += 1
		push_error(label)

func _run() -> void:
	var studio := Node3D.new()
	root.add_child(studio)
	var car := Node3D.new()
	car.physics_interpolation_mode = Node.PHYSICS_INTERPOLATION_MODE_OFF
	studio.add_child(car)
	var results := {}
	for script_path in ["res://tools/authoring_diagnostics/chase_camera_before.gd", "res://scripts/chase_camera.gd"]:
		var camera := Camera3D.new()
		camera.set_script(load(script_path))
		studio.add_child(camera)
		var distances := {}
		for rate in [30, 60, 144]:
			camera.reset_follow()
			car.basis = Basis(Vector3.UP, PI)
			car.position = Vector3.ZERO
			for i in rate * 5:
				car.position = Vector3(0, 0, 30.0 * i / rate)
				camera.follow(car, Vector3(0, 0, 30), 1.0 / rate)
			var behind := car.position.z - camera.position.z
			distances[str(rate)] = behind
			if script_path.ends_with("scripts/chase_camera.gd"):
				check(behind > 7.0 and behind < 9.0, "Chase lag/distance at " + str(rate))
		results[script_path] = distances
		if script_path.ends_with("scripts/chase_camera.gd"):
			camera.reset_follow()
			car.position = Vector3.ZERO
			car.basis = Basis(Vector3.UP, PI)
			camera.follow(car, Vector3(0, 0, -15), 1.0 / 60.0)
			check(camera.position.z < -6.0, "Reverse flips chase camera")
			var previous: Vector3 = -camera.basis.z
			car.basis = Basis.IDENTITY
			camera.follow(car, Vector3.ZERO, 1.0 / 60.0)
			check(previous.angle_to(-camera.basis.z) < 0.07, "Abrupt yaw is not bounded")
			car.basis = Basis(Vector3(0, 0, 1), PI) * Basis(Vector3.UP, PI)
			for _i in 120:
				camera.follow(car, Vector3.ZERO, 1.0 / 60.0)
			check(absf(camera.basis.x.y) < 0.0001, "Roll tilts the horizon")
			car.position = Vector3(500, 50, 600)
			camera.reset_follow()
			camera.follow(car, Vector3.ZERO, 1.0 / 60.0)
			check(camera.position.distance_to(car.position) < 9.0, "Respawn retains old camera location")
			car.position = Vector3.ZERO
			car.basis = Basis(Vector3.UP, PI)
			var wall := StaticBody3D.new()
			wall.collision_layer = TrackGeometry.TERRAIN_LAYER
			wall.position = Vector3(0, 2.0, -4.0)
			studio.add_child(wall)
			var collider := CollisionShape3D.new()
			collider.shape = BoxShape3D.new()
			collider.shape.size = Vector3(10, 6, 0.5)
			wall.add_child(collider)
			await physics_frame
			await physics_frame
			camera.reset_follow()
			camera.follow(car, Vector3.ZERO, 1.0 / 60.0)
			check(camera.position.z > -3.5, "Camera clips through terrain")
			wall.free()
			await physics_frame
			for _i in 120:
				camera.follow(car, Vector3.ZERO, 1.0 / 60.0)
			check(camera.position.z < -6.5, "Camera does not return after obstruction")
		camera.free()
	print("CAMERA_PROBE ", JSON.stringify({"failures": failures, "behind_car_m_at_30mps": results}))
	studio.free()
	quit(1 if failures else 0)
