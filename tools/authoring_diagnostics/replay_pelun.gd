extends SceneTree
func _initialize() -> void:
    _run.call_deferred()
func _run() -> void:
    GDExtensionManager.load_extension("res://.stage_authoring/native/accelerator.gdextension")
    var stage: RallyStage = load("res://resources/stages/chile-pelun-158408.res")
    stage.terrain = TerrainField.new()
    var field: TerrainField = stage.terrain
    field.initialize(stage, stage.terrain_settings)
    field.authoring_only = true
    field.native_authoring = ClassDB.instantiate("TerrainBakeAccelerator")
    field.native_authoring.configure(stage.centers, field._segments, stage.terrain_settings.amplitude, stage.terrain_settings.wavelength, field._seed, stage.road_width * 0.5, stage.terrain_settings.blend_distance)
    preload("res://scripts/road_shoulders.gd").new().build(stage)
    var chunk: Dictionary = load("res://.stage_authoring/pelun-failing-chunks.res").get_meta("chunks")[0].duplicate(true)
    field.chunks = {chunk.coord: chunk}
    field._chunk_data = [chunk]
    for attempt in 256:
        chunk.merge(TerrainField.Mesher.new().mesh_chunk(field, chunk), true)
        print("ATTEMPT ", attempt, " max=", chunk.max_error, " refine=", chunk.refine)
        if chunk.refine.is_empty():
            break
        var requested := {}
        for index in chunk.refine:
            var rect: Rect2i = chunk.leaves[index].rect
            print("BAD ", index, " ", rect)
            requested[rect] = true
            for point: Vector2i in [rect.position, Vector2i(rect.end.x, rect.position.y), Vector2i(rect.position.x, rect.end.y), rect.end]:
                for offset: Vector2i in [Vector2i.ZERO, Vector2i.LEFT, Vector2i.UP, -Vector2i.ONE]:
                    var neighbor := field.leaf_at(point + offset)
                    if not neighbor.is_empty() and neighbor.rect.size != Vector2i.ONE:
                        requested[neighbor.rect] = true
        chunk.needs_refine = PackedInt32Array()
        for i in chunk.leaves.size():
            if requested.has(chunk.leaves[i].rect):
                chunk.needs_refine.append(i)
        print("REQUESTED ", chunk.needs_refine)
        TerrainField.Mesher.new().refine_chunk(field, chunk)
    quit()
