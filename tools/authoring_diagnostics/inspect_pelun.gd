extends SceneTree
func _initialize() -> void:
    GDExtensionManager.load_extension("res://.stage_authoring/native/accelerator.gdextension")
    var native = ClassDB.instantiate("TerrainBakeAccelerator")
    native.configure(PackedVector3Array(), {}, 16, 220, 1, 3, 16)
    var chunks: Array = load("res://.stage_authoring/pelun-failing-chunks.res").get_meta("chunks")
    for chunk in chunks:
        print("CHUNK ", chunk.coord, " REFINE ", chunk.refine, " NEEDS ", chunk.needs_refine, " DIRTY ", chunk.mesh_dirty)
        for i in chunk.refine:
            var rect: Rect2i = chunk.leaves[i].rect
            print("BAD_RECT ", i, " ", rect, " requested ", chunk.needs_refine.has(i))
            var actual := PackedInt32Array([i])
            var result: Dictionary = native.refine_chunk(chunk.coord, chunk.samples, chunk.distances, chunk.leaves, actual)
            for leaf in result.leaves:
                if rect.intersects(leaf.rect):
                    print("REFINED ", leaf.rect)
    quit()
