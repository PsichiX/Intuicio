class_name ScriptPopToRegisterNode
extends ScriptNode

func _ready():
	title = "Pop to register"
	var value = data().data.Operation.PopToRegister.index if data() else 0
	var edit = add_property_int("Index", false, value)
	edit.connect("changed", self, "_changed_index")
	add_execute_in("In")
	add_execute_out("Out")

func _changed_index(v):
	if data():
		data().data.Operation.PopToRegister.index = v
		broadcast_change()
