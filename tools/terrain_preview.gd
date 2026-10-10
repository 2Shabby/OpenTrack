extends SceneTree

# Manual preview/profiling. Headless mode builds the same complete resident world.
# -- --length=500 --seed=1492 --amplitude=20 --wavelength=150 --gradient=0.1 --blend=64
func _initialize() -> void:
	_run.call_deferred()

func _capture(label: String) -> void:
	if DisplayServer.get_name() == "headless":
		return
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/opentrack-terrain-%s.png" % label)

func _run() -> void:
	var game := root.get_node("Game")
	var authored_stage: RallyStage
	for argument in OS.get_cmdline_user_args():
		var pair := argument.trim_prefix("--").split("=", false, 1)
		if pair.size() != 2:
			continue
		match pair[0]:
			"length": game.stage_length = int(pair[1])
			"seed": game.seed_value = int(pair[1])
			"stage":
				var catalog := StageCatalog.new()
				authored_stage = catalog.load_stage(pair[1])
				if authored_stage == null:
					push_error(catalog.error)
					quit(1)
					return
			"amplitude": game.terrain_settings.amplitude = float(pair[1])
			"wavelength": game.terrain_settings.wavelength = float(pair[1])
			"gradient": game.terrain_settings.max_gradient = float(pair[1])
			"blend": game.terrain_settings.blend_distance = float(pair[1])
	var started := Time.get_ticks_msec()
	var app: Node = load("res://scenes/main.tscn").instantiate()
	root.add_child(app)
	app.content.get_node("%Start").pressed.emit()
	await process_frame
	await _capture("setup")
	if authored_stage == null:
		app.content.get_node("%Start").pressed.emit()
	else:
		app._start_race(authored_stage)
	var world: Node3D = app.content
	while is_instance_valid(world) and world.generating and game.setup_error.is_empty():
		await process_frame
	if not game.setup_error.is_empty() or not is_instance_valid(world):
		push_error(game.setup_error)
		quit(1)
		return
	var build_ms := Time.get_ticks_msec() - started
	for _i in 120:
		await physics_frame
	await _capture("drive")
	Input.action_press("throttle_positive")
	var times: Array[float] = []
	var physics_times: Array[float] = []
	var previous := Time.get_ticks_usec()
	for _i in 180:
		await process_frame
		var now := Time.get_ticks_usec()
		times.append(float(now - previous) / 1000.0)
		physics_times.append(Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS) * 1000.0)
		previous = now
	Input.action_release("throttle_positive")
	times.sort()
	physics_times.sort()
	print("Stage preview: ", {"seed": game.seed_value, "length_m": world.stage.length_m, "build_ms": build_ms, "tracked_memory_bytes": OS.get_static_memory_usage(), "frame_p50_ms": times[times.size() / 2], "frame_p95_ms": times[floori(times.size() * 0.95)], "physics_p95_ms": physics_times[floori(physics_times.size() * 0.95)], "renderer": DisplayServer.get_name(), "terrain": world.stage.terrain.stats, "telemetry": world.car_root.telemetry()})
	world.set_process(false)
	world.set_physics_process(false)
	world.car_root.freeze_at_finish()
	world.get_node("HUD").visible = false
	var camera := Camera3D.new()
	world.add_child(camera)
	camera.current = true
	var pose: Transform3D = world.stage.road_pose(minf(320, world.stage.length_m * 0.5))
	camera.global_position = pose.origin + Vector3(65, 45, -90)
	camera.look_at(world.stage.road_pose(minf(380, world.stage.length_m * 0.65)).origin)
	for _i in 3:
		await process_frame
	await _capture("overview")
	print("Terrain preview complete; rendered captures use /tmp/opentrack-terrain-*.png")
	app.queue_free()
	await process_frame
	quit()
