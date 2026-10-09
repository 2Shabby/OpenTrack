extends SceneTree

func _initialize() -> void:
	_run.call_deferred()

func _run() -> void:
	var result := GDExtensionManager.load_extension("res://.stage_authoring/native/accelerator.gdextension")
	if result != GDExtensionManager.LOAD_STATUS_OK:
		push_error("Native accelerator did not load")
		quit(1)
		return
	var stage := ResourceLoader.load("res://resources/stages/wales-slate-mountain-17119.res", "", ResourceLoader.CACHE_MODE_IGNORE) as RallyStage
	var field := TerrainField.new()
	field.initialize(stage, stage.terrain_settings)
	var native: RefCounted = ClassDB.instantiate("TerrainBakeAccelerator")
	native.configure(stage.centers, field._segments, stage.terrain_settings.amplitude, stage.terrain_settings.wavelength, field._seed, stage.road_width * 0.5, stage.terrain_settings.blend_distance)
	var maximum := 0.0
	for center in stage.centers:
		for offset in [Vector2.ZERO, Vector2(3, 2), Vector2(25, -30)]:
			var point: Vector2 = Vector2(center.x, center.z) + offset
			var reference: Vector2 = field.sample(point)
			var accelerated: Vector2 = native.sample_point(point)
			if absf(reference.x - accelerated.x) > 0.00001:
				push_error("Native point sampling differs: %s" % point)
				quit(1)
				return
	for coord in [Vector2i.ZERO, Vector2i(-1, -1), field._chunk_data[-1].coord]:
		var slow_start := Time.get_ticks_msec()
		var reference: Dictionary = TerrainField.Mesher.new().sample_chunk(field, coord)
		var slow_ms := Time.get_ticks_msec() - slow_start
		var fast_start := Time.get_ticks_msec()
		var accelerated: Dictionary = native.sample_chunk(coord)
		var fast_ms := Time.get_ticks_msec() - fast_start
		if reference.has("error") or accelerated.has("error"):
			push_error("Sampling failed")
			quit(1)
			return
		for i in reference.samples.size():
			maximum = maxf(maximum, absf(reference.samples[i] - accelerated.samples[i]))
			if absf(reference.samples[i] - accelerated.samples[i]) > 0.00001 or absf(reference.distances[i] - accelerated.distances[i]) > maxf(0.00001, absf(reference.distances[i]) * 0.000001):
				push_error("Native source differs at %s:%s: %s versus %s" % [coord, i, [reference.samples[i], reference.distances[i]], [accelerated.samples[i], accelerated.distances[i]]])
				quit(1)
				return
		if reference.leaves.size() != accelerated.leaves.size() or reference.owners != accelerated.owners:
			push_error("Native partition differs at %s: %s versus %s" % [coord, reference.leaves.size(), accelerated.leaves.size()])
			quit(1)
			return
		print("NATIVE_PARITY ", coord, " · ", slow_ms, " ms reference · ", fast_ms, " ms native · ", accelerated.leaves.size(), " leaves")
	print("NATIVE_PARITY_OK max_height_difference=", maximum)
	quit()
