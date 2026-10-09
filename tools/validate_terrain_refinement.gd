extends SceneTree

func _initialize() -> void:
	_run.call_deferred()

func _run() -> void:
	var status := GDExtensionManager.load_extension("res://.stage_authoring/native/accelerator.gdextension")
	if status != GDExtensionManager.LOAD_STATUS_OK:
		push_error("Native accelerator did not load")
		quit(1)
		return
	var reference: Array = []
	for incremental in [false, true]:
		var stage := ResourceLoader.load("res://resources/stages/wales-slate-mountain-17119.res", "", ResourceLoader.CACHE_MODE_IGNORE) as RallyStage
		stage.terrain = TerrainField.new()
		stage.terrain.initialize(stage, stage.terrain_settings)
		stage.terrain.authoring_only = true
		stage.terrain.set_meta("full_remesh", not incremental)
		stage.terrain.native_authoring = ClassDB.instantiate("TerrainBakeAccelerator")
		stage.terrain.native_authoring.configure(stage.centers, stage.terrain._segments, stage.terrain_settings.amplitude, stage.terrain_settings.wavelength, stage.terrain._seed, stage.road_width * 0.5, stage.terrain_settings.blend_distance)
		var geometry := TrackGeometry.new()
		root.add_child(geometry)
		if not await geometry.author(stage):
			push_error(stage.terrain.error)
			quit(1)
			return
		for i in stage.terrain.meshes.size():
			var arrays: Array = stage.terrain.meshes[i].surface_get_arrays(0)
			if incremental:
				if reference[i][Mesh.ARRAY_VERTEX] != arrays[Mesh.ARRAY_VERTEX] or reference[i][Mesh.ARRAY_NORMAL] != arrays[Mesh.ARRAY_NORMAL]:
					push_error("Incremental terrain differs in chunk " + str(i))
					quit(1)
					return
			else:
				reference.append(arrays)
		print("REFINEMENT_VARIANT ", incremental, " · ", stage.terrain.stats)
		geometry.queue_free()
		stage = null
		await process_frame
		await physics_frame
	print("TERRAIN_REFINEMENT_PARITY_OK ", reference.size(), " chunks, identical vertices/normals")
	quit()
