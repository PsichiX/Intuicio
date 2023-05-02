class_name ScriptDropRegisterNode
extends ScriptNode

func _ready():
	title = "Drop register"
	var value = data().data.Operation.DropRegister.index if data() else 0
	var edit = add_property_int("Index", false, value)
	edit.connect("changed", self, "_changed_index")
	add_execute_in("In")
	add_execute_out("Out")

func _changed_index(v):
	if data():
		data().data.Operation.DropRegister.index = v
		broadcast_change()
