extends SceneTree

# Render check for terrain and setup, using the actual application scenes.
func _initialize() -> void:
	_run.call_deferred()

func _run() -> void:
	var app: Node = load("res://scenes/main.tscn").instantiate()
	root.add_child(app)
	app.content.get_node("%Start").pressed.emit()
	await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/opentrack-terrain-setup.png")
	app.content.get_node("%Start").pressed.emit()
	var world: Node3D = app.content
	for _i in 6:
		await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/opentrack-terrain-drive.png")
	world.set_process(false)
	world.set_physics_process(false)
	world.get_node("HUD").visible = false
	var camera := Camera3D.new()
	world.add_child(camera)
	camera.current = true
	var pose: Transform3D = world.stage.road_pose(320)
	camera.global_position = pose.origin + Vector3(65, 45, -90)
	camera.look_at(world.stage.road_pose(380).origin)
	for _i in 3:
		await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/opentrack-terrain-overview.png")
	print("Terrain preview saved to /tmp/opentrack-terrain-{setup,drive,overview}.png")
	quit()
