extends SceneTree
func _initialize() -> void:
	_run.call_deferred()
func _run() -> void:
	GDExtensionManager.load_extension("res://.stage_authoring/native/accelerator.gdextension")
	var stage := StageCatalog.new().load_stage("australia-orara-east-11997")
	var field := TerrainField.new()
	field.initialize(stage,stage.terrain_settings)
	var native: RefCounted=ClassDB.instantiate("TerrainBakeAccelerator")
	native.configure(stage.centers,field._segments,stage.terrain_settings.amplitude,stage.terrain_settings.wavelength,field._seed,stage.road_width*0.5,stage.terrain_settings.blend_distance)
	for coord in [Vector2i(-26,80),Vector2i(35,123)]:
		var c: Dictionary=native.sample_chunk(coord)
		var leaf: Dictionary=c.leaves[c.owners[160*320+160]]
		var r: Rect2i=leaf.rect
		var p:=PackedVector2Array([Vector2(r.position)*0.1,Vector2(r.end.x,r.position.y)*0.1,Vector2(r.position.x,r.end.y)*0.1])
		var local:=PackedVector2Array([Vector2.ZERO,p[1]-p[0],p[2]-p[0]])
		print("LEAF ",r," GLOBAL ",p," INDICES ",Geometry2D.triangulate_polygon(p)," LOCAL ",Geometry2D.triangulate_polygon(local))
	quit()
