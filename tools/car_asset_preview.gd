extends SceneTree

# Native renderer checks for the authored model, paint isolation and lamp states.
# godot --path . --script tools/car_asset_preview.gd
func _initialize() -> void:
	_run.call_deferred()

func _capture(name: String) -> void:
	for _i in 4:
		await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/opentrack-car-%s.png" % name)

func _run() -> void:
	var game := root.get_node("Game")
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--vehicle="):
			game.car_scene = load(argument.trim_prefix("--vehicle="))
	var studio := Node3D.new()
	root.add_child(studio)
	var environment := WorldEnvironment.new()
	environment.environment = Environment.new()
	environment.environment.background_mode = Environment.BG_COLOR
	environment.environment.background_color = Palette.color(24)
	environment.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	environment.environment.ambient_light_color = Palette.color(19)
	environment.environment.ambient_light_energy = 0.65
	studio.add_child(environment)
	var light := DirectionalLight3D.new()
	light.rotation_degrees = Vector3(-45, -35, 0)
	light.light_energy = 1.5
	light.shadow_enabled = true
	studio.add_child(light)
	var floor_body := StaticBody3D.new()
	floor_body.collision_layer = TrackGeometry.ROAD_LAYER
	floor_body.add_to_group(&"Road")
	floor_body.position.y = -0.1
	studio.add_child(floor_body)
	var floor_shape := CollisionShape3D.new()
	floor_shape.shape = BoxShape3D.new()
	floor_shape.shape.size = Vector3(30, 0.2, 30)
	floor_body.add_child(floor_shape)
	var floor_mesh := MeshInstance3D.new()
	floor_mesh.mesh = BoxMesh.new()
	floor_mesh.mesh.size = floor_shape.shape.size
	var floor_material := StandardMaterial3D.new()
	floor_material.albedo_color = Palette.color(23)
	floor_mesh.material_override = floor_material
	floor_body.add_child(floor_mesh)
	var cars: Array[RallyCar] = []
	for i in 2:
		var car: RallyCar = game.car_scene.instantiate()
		car.accept_input = false
		car.configure(game.player_paint_index(i))
		if not car.place_at(Transform3D(Basis.IDENTITY, Vector3(-1.8 + i * 3.6, 0, 0))):
			push_error(car.configuration_error)
			quit(1)
			return
		studio.add_child(car)
		cars.append(car)
	for _i in 240:
		await physics_frame
	for car in cars:
		car.freeze_at_finish()
	var camera := Camera3D.new()
	studio.add_child(camera)
	camera.current = true
	camera.fov = 42
	var target := Vector3(0, 0.65, 0)
	camera.position = Vector3(7, 4, 9)
	camera.look_at(target)
	await _capture("front-colors")
	camera.position = Vector3(-7, 3.2, -9)
	camera.look_at(target)
	await _capture("rear-running")
	cars[0].visual.set_lights(1, false)
	cars[1].visual.set_lights(0, true)
	await _capture("rear-brake-reverse")
	camera.position = Vector3(8, 2.5, 0)
	camera.look_at(cars[1].position + Vector3.UP * 0.65)
	await _capture("side")
	print("car studio captured: front, rear running, brake/reverse, side")
	studio.queue_free()
	await process_frame
	quit()
