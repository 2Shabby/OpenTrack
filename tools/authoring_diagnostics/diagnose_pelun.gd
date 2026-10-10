extends SceneTree
func _initialize() -> void:
    _run.call_deferred()
func _run() -> void:
    GDExtensionManager.load_extension("res://.stage_authoring/native/accelerator.gdextension")
    var stage: RallyStage = load("res://resources/stages/chile-pelun-158408.res")
    stage.terrain = load("res://tools/authoring_diagnostics/diagnostic_field.gd").new()
    stage.terrain.initialize(stage, stage.terrain_settings)
    stage.terrain.authoring_only = true
    stage.terrain.native_authoring = ClassDB.instantiate("TerrainBakeAccelerator")
    stage.terrain.native_authoring.configure(stage.centers, stage.terrain._segments, stage.terrain_settings.amplitude, stage.terrain_settings.wavelength, stage.terrain._seed, stage.road_width * 0.5, stage.terrain_settings.blend_distance)
    var geometry := TrackGeometry.new()
    root.add_child(geometry)
    var result := await geometry.author(stage)
    var snapshot := Resource.new()
    var failing: Array = []
    for chunk in stage.terrain._chunk_data:
        if not chunk.get("refine", []).is_empty():
            failing.append(chunk)
    snapshot.set_meta("chunks", failing)
    ResourceSaver.save(snapshot, "res://.stage_authoring/pelun-failing-chunks.res", ResourceSaver.FLAG_COMPRESS)
    print("DIAGNOSTIC_COMPLETE ", result, " ", stage.terrain.error)
    quit(0 if result else 1)
