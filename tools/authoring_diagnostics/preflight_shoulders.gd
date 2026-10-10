extends SceneTree
const FIELD = preload("res://scripts/terrain_field.gd")
func _initialize() -> void:
    _run.call_deferred()
func _run() -> void:
    GDExtensionManager.load_extension("res://.stage_authoring/native/accelerator.gdextension")
    var entries := StageCatalog.new().entries
    entries.sort_custom(func(a, b): return a.length_m > b.length_m)
    var ids := [entries[0].id, "finland-assamaki-125404", "portugal-fafe-14722"]
    for id in ids:
        var entry := StageCatalog.new().entry_for(id)
        var stage: RallyStage = load(entry.path)
        stage.terrain = FIELD.new()
        stage.terrain.initialize(stage, stage.terrain_settings)
        stage.terrain.authoring_only = true
        stage.terrain.native_authoring = ClassDB.instantiate("TerrainBakeAccelerator")
        stage.terrain.native_authoring.configure(stage.centers, stage.terrain._segments, stage.terrain_settings.amplitude, stage.terrain_settings.wavelength, stage.terrain._seed, stage.road_width * 0.5, stage.terrain_settings.blend_distance)
        var geometry := TrackGeometry.new()
        root.add_child(geometry)
        for piece: Dictionary in stage.pieces():
            var arrays: Array = []
            arrays.resize(Mesh.ARRAY_MAX)
            arrays[Mesh.ARRAY_VERTEX] = piece["vertices"]
            arrays[Mesh.ARRAY_NORMAL] = piece["normals"]
            arrays[Mesh.ARRAY_TEX_UV] = piece["uvs"]
            arrays[Mesh.ARRAY_INDEX] = piece["indices"]
            var mesh := ArrayMesh.new()
            mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
            geometry._add_surface(mesh, mesh.create_trimesh_shape(), TrackGeometry.SURFACES[int(piece["surface"])], TrackGeometry.ROAD_LAYER)
        geometry._add_shoulders(stage)
        if false:
            _own(geometry, geometry)
            var saved := PackedScene.new()
            saved.pack(geometry)
            ResourceSaver.save(saved, "res://.stage_authoring/longest-shoulders.scn", ResourceSaver.FLAG_COMPRESS)
        await physics_frame
        var space := geometry.get_world_3d().direct_space_state
        var probes := 0
        for i in range(0, stage.centers.size(), 25):
            for edge: Vector3 in [stage.left_edges[i], stage.right_edges[i]]:
                var point := edge + (edge - stage.centers[i]).normalized() * 0.1
                var query := PhysicsRayQueryParameters3D.create(point + Vector3.UP * 1000, point - Vector3.UP * 1000, TrackGeometry.SUPPORT_MASK)
                if not is_finite(preload("res://scripts/baked_stage_terrain.gd").ray_support(space, query)):
                    print("MISSING_POINT ", point, " center ", stage.centers[i], " left ", stage.left_edges[i], " right ", stage.right_edges[i])
                    for offset in [Vector3.ZERO, Vector3(0.005, 0, 0), Vector3(-0.005, 0, 0), Vector3(0, 0, 0.005), Vector3(0, 0, -0.005)]:
                        for reach in [1000.0, 10.0, 1.0]:
                            var test: Vector3 = point + offset
                            var hit := space.intersect_ray(PhysicsRayQueryParameters3D.create(test + Vector3.UP * reach, test - Vector3.UP * reach, TrackGeometry.SUPPORT_MASK))
                            print("NEARBY_RAY ", offset, " reach ", reach, " ", hit.get("position", "missing"))
                    push_error("SHOULDER_PREFLIGHT_MISSING " + id + " station " + str(i))
                    quit(1)
                    return
                probes += 1
        print("SHOULDER_PREFLIGHT_OK ", id, " · ", probes, " native verge probes")
        geometry.queue_free()
        stage.terrain = null
        await process_frame
        await physics_frame
    print("SHOULDER_PREFLIGHT_ALL_OK")
    quit()
func _own(node: Node, scene: Node) -> void:
    for child in node.get_children():
        child.owner = scene
        _own(child, scene)
