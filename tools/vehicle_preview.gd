extends SceneTree

# Render the real application at normal speed. This is scripted visual validation,
# not a substitute for a human driving/handling pass.
# -- --stage=wales-slate-mountain-17119 selects a saved world.
func _initialize() -> void:
	_run.call_deferred()

func _frames(count: int) -> void:
	for _i in count:
		await physics_frame

func _capture(label: String) -> void:
	if DisplayServer.get_name() == "headless":
		return
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/opentrack-vehicle-%s.png" % label)

func _run() -> void:
	var game := root.get_node("Game")
	game.stage_mode = game.StageMode.PROCEDURAL_TEST
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--vehicle="):
			game.car_scene = load(argument.trim_prefix("--vehicle="))
		if argument.begins_with("--stage="):
			game.stage_mode = game.StageMode.SAVED
			game.selected_stage_id = argument.trim_prefix("--stage=")
	var app: Node = load("res://scenes/main.tscn").instantiate()
	root.add_child(app)
	app.content.get_node("%Start").pressed.emit()
	await process_frame
	app.content.get_node("%Start").pressed.emit()
	var world: Node3D = app.content
	while is_instance_valid(world) and world.generating and game.setup_error.is_empty():
		await process_frame
	if not game.setup_error.is_empty() or not is_instance_valid(world):
		push_error(game.setup_error)
		quit(1)
		return
	await _frames(360)
	await _capture("settled")
	Input.action_press("throttle_positive")
	await _frames(360)
	await _capture("driving")
	Input.action_release("throttle_positive")
	Input.action_press("steer_left")
	await _frames(60)
	Input.action_release("steer_left")
	await _capture("steering")
	world.car_root.apply_central_impulse(Vector3.UP * world.car_root.mass * 6)
	world.car_root.angular_velocity = Vector3(0.25, 0.15, 0.2)
	await _frames(48)
	await _capture("airborne")
	world.set_paused(true)
	await _capture("paused")
	world.set_paused(false)
	var landed := false
	for _i in 1200:
		await physics_frame
		if world.car_root.contact_count() >= 3:
			landed = true
			break
	await _capture("landed" if landed else "after-jump")
	print("rendered vehicle: ", world.car_root.telemetry(), " recoveries ", world.recovery_count)
	app.queue_free()
	await process_frame
	quit()
