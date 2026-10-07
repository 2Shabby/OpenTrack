extends Control

func update_run(driver: String, drivers: int, speed: float, progress: float, length_m: float, note: String, elapsed: float, finished: bool, best: float, surface: String) -> void:
	%Session.text = "%s / %d drivers · %.0f km/h · %s · %d / %d m" % [driver, drivers, speed * 3.6, surface.capitalize(), roundi(progress), roundi(length_m)]
	%Pacenote.text = note
	%Time.text = "%s%.2f s%s" % ["Finished · " if finished else "Time · ", elapsed, " · Best %.2f s" % best if best > 0 else ""]
