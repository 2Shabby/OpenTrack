extends SceneTree
func _initialize() -> void:
    var catalog := StageCatalog.new()
    var updated := 0
    for entry in catalog.entries:
        var stage: RallyStage = ResourceLoader.load(entry.path, "", ResourceLoader.CACHE_MODE_IGNORE)
        var previous: String = preload("res://tools/authoring_diagnostics/previous_fingerprint.gd").compute(stage)
        var middle: String = preload("res://tools/authoring_diagnostics/mid_fingerprint.gd").compute(stage)
        var current: String = preload("res://tools/stage_bake_fingerprint.gd").compute(stage)
        if stage.baked_fingerprint not in [previous, middle] or stage.baked_fingerprint == current or not ResourceLoader.exists(stage.baked_scene_path):
            continue
        stage.baked_stats["geometry_input_fingerprint"] = stage.baked_fingerprint
        stage.baked_fingerprint = current
        stage.baked_stats["successful_geometry_preserved"] = true
        var temporary: String = entry.path.trim_suffix(".res") + ".pending.res"
        if ResourceSaver.save(stage, temporary, ResourceSaver.FLAG_COMPRESS) != OK or DirAccess.rename_absolute(temporary, entry.path) != OK:
            push_error("Cannot save provenance " + entry.id)
            quit(1)
            return
        entry.baked_fingerprint = current
        entry.baked_stats = stage.baked_stats
        updated += 1
    var index := Resource.new()
    index.set_meta("version", 1)
    index.set_meta("stages", catalog.entries)
    if ResourceSaver.save(index, "res://resources/stages/catalog.pending.tres") != OK or DirAccess.rename_absolute("res://resources/stages/catalog.pending.tres", StageCatalog.INDEX_PATH) != OK:
        push_error("Cannot save catalog provenance")
        quit(1)
        return
    var file := FileAccess.open("res://resources/stages/catalog.pending.json", FileAccess.WRITE)
    file.store_string(JSON.stringify({"version": 1, "stages": catalog.entries}, "\t") + "\n")
    file.close()
    if DirAccess.rename_absolute("res://resources/stages/catalog.pending.json", "res://resources/stages/catalog.json") != OK:
        push_error("Cannot publish JSON provenance")
        quit(1)
        return
    print("PROVENANCE_UPGRADED ", updated, " worlds; geometry unchanged")
    quit()
