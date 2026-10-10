extends SceneTree
func _initialize() -> void:
    var polygon := PackedVector2Array([Vector2(0.0,0.0),Vector2(-0.064941,0.0),Vector2(-0.081177,-0.018555),Vector2(-0.099854,-0.040039),Vector2(-0.127808,-0.072266),Vector2(-0.1521,-0.100098),Vector2(-0.174316,-0.125488),Vector2(-0.200073,-0.155273),Vector2(-0.220947,-0.179199),Vector2(-0.239136,-0.200195),Vector2(-0.267334,-0.232422),Vector2(-0.300049,-0.27002),Vector2(-0.314087,-0.286133),Vector2(-0.32605,-0.299805),Vector2(-0.360474,-0.339355),Vector2(-0.400024,-0.384766),Vector2(-0.407104,-0.393066),Vector2(-0.413208,-0.399902),Vector2(-0.453613,-0.446289),Vector2(-0.500122,-0.5),Vector2(-0.499878,-0.499512)])
    print("BASE ", Geometry2D.triangulate_polygon(polygon))
    var cleaned := polygon.duplicate()
    cleaned.remove_at(19)
    print("REMOVE19 ", Geometry2D.triangulate_polygon(cleaned))
    cleaned = polygon.duplicate()
    cleaned.remove_at(20)
    print("REMOVE20 ", Geometry2D.triangulate_polygon(cleaned))
    quit()
