extends SceneTree
func _initialize() -> void:
	_run.call_deferred()
func _run() -> void:
	var stage := StageCatalog.new().load_stage("australia-orara-east-11997")
	var geometry := TrackGeometry.new()
	root.add_child(geometry)
	await geometry.build(stage)
	for point in [Vector2(-816,2576),Vector2(1104,3920),Vector2(1136,3952),Vector2(1136,3984),Vector2(1168,4016)]:
		print("POINT ",point," ray=",stage.terrain.height_at(point))
		var nearest := INF
		for center in stage.centers:
			nearest=minf(nearest,point.distance_to(Vector2(center.x,center.z)))
		print("ROAD_DISTANCE ",nearest)
		for offset in [Vector2(0.001,0),Vector2(0,0.001),Vector2(-0.001,0),Vector2(0,-0.001),Vector2(0.01,0.01),Vector2(1,1)]:
			print("OFFSET ",offset," height=",stage.terrain.height_at(point+offset))
		var count := 0
		for visual in geometry.find_children("*","MeshInstance3D",true,false):
			if not visual.mesh is ArrayMesh:
				continue
			var aabb: AABB = visual.get_aabb()
			if point.x<aabb.position.x or point.x>aabb.end.x or point.y<aabb.position.z or point.y>aabb.end.z:
				continue
			var arrays: Array = visual.mesh.surface_get_arrays(0)
			var v: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
			var ids: PackedInt32Array = arrays[Mesh.ARRAY_INDEX] if arrays[Mesh.ARRAY_INDEX] != null else PackedInt32Array()
			for i in range(0,v.size() if ids.is_empty() else ids.size(),3):
				var a: Vector3=v[i] if ids.is_empty() else v[ids[i]]
				var b: Vector3=v[i+1] if ids.is_empty() else v[ids[i+1]]
				var c: Vector3=v[i+2] if ids.is_empty() else v[ids[i+2]]
				var h := TerrainField.triangle_height(point,a,b,c)
				if is_finite(h):
					count+=1
					print("CPU_TRIANGLE ",a," ",b," ",c," height=",h)
		print("CPU_HITS ",count)
	quit()
