extends CanvasLayer

signal toggle_pause
signal resume_race
signal restart_race
signal next_driver
signal open_setup
signal open_menu
signal quit_game

func _ready() -> void:
	%Resume.pressed.connect(func() -> void: resume_race.emit())
	%Restart.pressed.connect(func() -> void: restart_race.emit())
	%NextDriver.pressed.connect(func() -> void: next_driver.emit())
	%Setup.pressed.connect(func() -> void: open_setup.emit())
	%Menu.pressed.connect(func() -> void: open_menu.emit())
	%Quit.pressed.connect(func() -> void: quit_game.emit())
	visibility_changed.connect(func() -> void:
		if visible:
			%Resume.grab_focus()
	)

func _process(_delta: float) -> void:
	if Input.is_action_just_pressed("pause_game"):
		toggle_pause.emit()
