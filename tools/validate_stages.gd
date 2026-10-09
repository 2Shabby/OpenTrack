extends SceneTree

var failures := 0
var ready_count := 0

func _initialize() -> void:
	_validate.call_deferred()

func check(condition: bool, message: String) -> void:
	if not condition:
		failures += 1
		push_error(message)

func _validate() -> void:
	var catalog := StageCatalog.new()
	check(catalog.error.is_empty(), catalog.error)
	check(catalog.entries.size() == 50, "Expected exactly 50 saved stages")
	var ids: Array[String] = []
	var total := 0.0
	for entry in catalog.entries:
		ids.append(entry.id)
		var stage := ResourceLoader.load(entry.path, "", ResourceLoader.CACHE_MODE_IGNORE) as RallyStage
		check(stage != null, "Could not load " + entry.id)
		if stage == null:
			continue
		check(stage.generator_version == "rally2gpx-authored-v1", "Not an authored stage: " + entry.id)
		check(stage.source_sha256 == FileAccess.get_sha256(stage.source_gpx), "Source hash differs: " + entry.id)
		check(absf(stage.length_m - stage.source_length_m) / stage.source_length_m < 0.011, "Route length changed: " + entry.id)
		check(stage.centers.size() == stage.headings.size() and stage.centers.size() == stage.distances.size(), "Station arrays differ: " + entry.id)
		check(stage.left_edges.size() == stage.centers.size() and stage.right_edges.size() == stage.centers.size(), "Missing road edges: " + entry.id)
		if not stage.baked_scene_path.is_empty():
			ready_count += 1
			check(ResourceLoader.exists(stage.baked_scene_path), "Missing baked world: " + entry.id)
			check(not stage.baked_fingerprint.is_empty() and stage.baked_bounds.size != Vector3.ZERO, "Missing bake metadata: " + entry.id)
		var last_station := 0
		for feature in stage.features:
			check(feature.first_station == last_station and feature.last_station > last_station, "Gap in road features: " + entry.id)
			check(feature.surface == 0 or feature.surface == 1, "Invalid surface: " + entry.id)
			if feature.kind == "corner":
				check(feature.grade >= 1 and feature.grade <= 6, "Invalid pacenote: " + entry.id)
			last_station = feature.last_station
		check(last_station == stage.centers.size() - 1, "Features do not reach finish: " + entry.id)
		for i in range(stage.centers.size() - 1):
			check(stage.centers[i].is_finite() and stage.distances[i+1] > stage.distances[i], "Invalid station: " + entry.id)
			var left := stage.left_edges[i]
			var right := stage.right_edges[i]
			check((stage.left_edges[i+1]-left).cross(right-left).y > 0.0, "Folded road triangle: " + entry.id)
			check((stage.left_edges[i+1]-right).cross(stage.right_edges[i+1]-right).y > 0.0, "Folded second triangle: " + entry.id)
		var finish := stage.road_pose(stage.length_m)
		check(stage.crossed_finish(finish * Vector3(0, 0, -1), finish * Vector3(0, 0, 1)), "Finish crossing failed: " + entry.id)
		check(not stage.crossed_finish(finish * Vector3(stage.road_width, 0, -1), finish * Vector3(stage.road_width, 0, 1)), "Off-road finish accepted: " + entry.id)
		check(not stage.crossed_finish(finish * Vector3(0, 0, 1), finish * Vector3(0, 0, -1)), "Reverse finish accepted: " + entry.id)
		check(stage.spawn_pose().basis.determinant() > 0.99, "Invalid spawn basis: " + entry.id)
		total += stage.length_m
	if "--require-baked" in OS.get_cmdline_user_args():
		check(ready_count == 50, "Expected 50 baked worlds; found %d" % ready_count)
	var previous := ""
	for _cycle in 3:
		var seen := {}
		for _i in ids.size():
			var id := catalog.draw(ids)
			check(id != previous, "Randomiser repeated consecutive stages")
			check(not seen.has(id), "Randomiser repeated within a bag")
			check(id in ids, "Randomiser returned unknown stage")
			seen[id] = true
			previous = id
	var subset: Array[String] = [ids[0], ids[1]]
	check(catalog.draw(subset) in subset, "Randomiser ignored changed filters")
	check(catalog.draw([]).is_empty(), "Empty pool should not pick a stage")
	var game := root.get_node("Game")
	game.stage_region = "All regions"
	game.stage_length_filter = 0
	game.selected_stage_id = ""
	var setup: Control = load("res://scenes/ui/setup_menu.tscn").instantiate()
	root.add_child(setup)
	check(setup.get_node("%Start").disabled == (ready_count == 0), "Saved setup availability differs from authored worlds")
	setup._validate_seed("9223372036854775808")
	check(setup.get_node("%Start").disabled == (ready_count == 0), "Saved stage availability should ignore procedural seed")
	setup.get_node("%Mode").item_selected.emit(1)
	check(setup.get_node("%Start").disabled, "Procedural setup accepted overflowing seed")
	setup._validate_seed("1592598566")
	check(not setup.get_node("%Start").disabled, "Valid procedural seed was rejected")
	game.stage_region = "No such region"
	setup.get_node("%Mode").item_selected.emit(0)
	setup._populate_stages()
	check(setup.get_node("%Start").disabled, "Empty filter pool should disable Start")
	setup.queue_free()
	game.stage_region = "All regions"
	game.stage_length_filter = 0
	if "--world" in OS.get_cmdline_user_args() and failures == 0:
		await _validate_world(game, catalog)
	print("Saved stage validation: %d failures; %d stages; %d baked worlds; %.1f km; three shuffle cycles" % [failures, catalog.entries.size(), ready_count, total / 1000.0])
	quit(1 if failures else 0)

func _validate_world(game: Node, catalog: StageCatalog) -> void:
	var shortest: Dictionary = {}
	for entry in catalog.entries:
		if not entry.get("baked_scene_path", "").is_empty() and (shortest.is_empty() or entry.length_m < shortest.length_m):
			shortest = entry
	check(not shortest.is_empty(), "No baked world is available to validate")
	if shortest.is_empty():
		return
	game.stage_mode = game.StageMode.SAVED
	game.selected_stage_id = shortest.id
	game.configure_players(2)
	game.state = game.State.DRIVING
	var world: Node3D = load("res://scenes/world.tscn").instantiate()
	root.add_child(world)
	var started := Time.get_ticks_msec()
	check(await world.start_race(), "Saved world failed: " + game.setup_error)
	if world.car_root == null:
		world.queue_free()
		return
	var stage: Resource = world.stage
	check(stage.terrain.get_script().resource_path == "res://scripts/baked_stage_terrain.gd", "Saved world generated runtime terrain")
	var bodies := world.get_node("Track").find_children("*", "StaticBody3D", true, false)
	check(not bodies.is_empty(), "Saved world contains no native collision")
	for body in bodies:
		check(body.get_groups().size() == 1 and body.has_meta("surface"), "Saved surface lost its tire friction group/profile")
	check(absf(stage.terrain.height_at(Vector2(stage.centers[2].x, stage.centers[2].z))-stage.centers[2].y) < 0.05, "Saved support query differs from road plane")
	for _i in 120:
		await physics_frame
	check(world.car_root.contact_count() >= 3, "Car did not settle on saved road")
	Input.action_press("throttle_positive")
	for _i in 90:
		await physics_frame
	Input.action_release("throttle_positive")
	check(world._elapsed > 0 and world.car_root.telemetry().speed > 0, "Saved stage driving/timer did not start")
	world.set_paused(true)
	var elapsed: float = world._elapsed
	for _i in 10:
		await process_frame
	check(world._elapsed == elapsed, "Pause advanced the run timer")
	world.set_paused(false)
	world._finish()
	check(world._best_times.has(0), "Finish did not save best time")
	world._next_driver()
	check(world.stage == stage and game.player_index == 1 and world._elapsed == 0, "Hotseat changed the track or failed to reset")
	world._restart()
	check(world.stage == stage and world._best_times.has(0), "Retry changed stage or cleared other driver's best")
	print("Saved world validation: ", shortest.id, " · ", Time.get_ticks_msec() - started, " ms · ", stage.terrain.stats)
	world.queue_free()
	await process_frame
