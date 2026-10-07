extends SceneTree

func _initialize() -> void:
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://assets/textures"))
	_save("asphalt", _asphalt)
	_save("dirt", _dirt)
	quit()

func _save(name: String, painter: Callable) -> void:
	var image := Image.create(256, 256, false, Image.FORMAT_RGB8)
	for y in 256:
		for x in 256:
			image.set_pixel(x, y, painter.call(x, y))
	var path := "res://assets/textures/%s.png" % name
	var error := image.save_png(path)
	print(path, " ", error)

func _hash(ix: int, iy: int, salt: int) -> float:
	var x := posmod(ix, 8)
	var y := posmod(iy, 8)
	var n := x * 374761393 + y * 668265263 + salt * 1442695041
	n = (n ^ (n >> 13)) * 1274126177
	return float(n & 0x7fffffff) / float(0x7fffffff)

func _noise(x: int, y: int, salt: int) -> float:
	var fx := float(x) / 32.0
	var fy := float(y) / 32.0
	var ix := int(floor(fx))
	var iy := int(floor(fy))
	var tx := fx - float(ix)
	var ty := fy - float(iy)
	tx = tx * tx * (3.0 - 2.0 * tx)
	ty = ty * ty * (3.0 - 2.0 * ty)
	var a := lerpf(_hash(ix, iy, salt), _hash(ix + 1, iy, salt), tx)
	var b := lerpf(_hash(ix, iy + 1, salt), _hash(ix + 1, iy + 1, salt), tx)
	return lerpf(a, b, ty)

func _mix(base: Color, amount: float) -> Color:
	return Color(
		clampf(base.r + amount, 0.0, 1.0),
		clampf(base.g + amount, 0.0, 1.0),
		clampf(base.b + amount, 0.0, 1.0)
	)

func _asphalt(x: int, y: int) -> Color:
	var n := (_noise(x, y, 3) - 0.5) * 0.08
	var grain := (_noise(x * 2, y * 2, 9) - 0.5) * 0.03
	var color := _mix(Color(0.16, 0.17, 0.16), n + grain)
	var lane := absf(float(x) - 128.0)
	var dash := posmod(y, 48) < 22
	if lane < 3.0 and dash:
		color = Color(0.82, 0.78, 0.45)
	if float(x) < 10.0 or float(x) > 246.0:
		color = Color(0.72, 0.72, 0.68)
	return color

func _dirt(x: int, y: int) -> Color:
	var n := (_noise(x, y, 11) - 0.5) * 0.16
	var patch := (_noise(x, y, 17) - 0.5) * 0.08
	return _mix(Color(0.42, 0.28, 0.14), n + patch)
