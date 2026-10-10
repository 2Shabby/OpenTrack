extends SceneTree
func _initialize() -> void:
    _run.call_deferred()
func _run() -> void:
    var scene = load("res://resources/stages/baked/portugal-fafe-14722.scn").instantiate()
    var geometry := TrackGeometry.new()
    root.add_child(geometry)
    for old_body in scene.get_node("Shoulders").get_children():
        var mesh: Mesh = old_body.get_child(0).mesh
        var body := geometry._add_surface(mesh, null, TrackGeometry.GRASS, TrackGeometry.TERRAIN_LAYER)
        geometry._add_shoulder_collision(body, mesh)
        print("SHOULDER_COLLISION ", body.get_child_count() - 1, " shapes · ", mesh.get_faces().size() / 3, " triangles")
    await physics_frame
    print("PARTITIONED_SHOULDERS_OK")
    scene.free()
    quit()
