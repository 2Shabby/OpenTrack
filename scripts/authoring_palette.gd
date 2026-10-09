class_name AuthoringPalette
extends Resource

@export var colors := PackedColorArray()

func color(index: int, alpha := 1.0) -> Color:
	assert(colors.size() == 32 and index >= 0 and index < 32, "Invalid authoring palette/index")
	var result := colors[index]
	result.a = alpha
	return result
