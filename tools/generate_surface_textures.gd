extends SceneTree

# Palette-only authoring: 10 cm texels, with no interpolated RGB.
func _initialize() -> void:
	var ramps := {"asphalt": [25, 24, 23], "dirt": [5, 0, 4, 1], "grass": [14, 13, 12]}
	for surface: String in ramps:
		var noise := FastNoiseLite.new()
		noise.seed = 32 + surface.length()
		noise.frequency = 0.18
		var image := Image.create(32, 32, false, Image.FORMAT_RGB8)
		var ramp: Array = ramps[surface]
		for z in 32:
			for x in 32:
				var level := clampi(floori((noise.get_noise_2d(x, z) + 1.0) * 0.5 * ramp.size()), 0, ramp.size() - 1)
				image.set_pixel(x, z, Palette.color(ramp[level]))
		var error := image.save_png("res://assets/textures/%s.png" % surface)
		if error != OK:
			push_error("Could not author surface texture: %s" % surface)
			quit(1)
			return
	quit()
