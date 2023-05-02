class_name ScriptMakeRegisterNode
extends ScriptNode

func _ready():
	title = "Make register"
	var value = data().data.Operation.MakeRegister.name if data() else ""
	var edit = add_property_string("Struct name", false, value)
	edit.connect("changed", self, "_changed_struct_name")
	value = data().data.Operation.MakeRegister.module_name if data() && "module_name" in data().data.Operation.MakeRegister else ""
	edit = add_property_string("Struct module name", false, value)
	edit.connect("changed", self, "_changed_struct_module_name")
	add_execute_in("In")
	add_execute_out("Out")

func _changed_struct_name(v):
	if data():
		data().data.Operation.MakeRegister.name = v
		broadcast_change()

func _changed_struct_module_name(v):
	if data():
		data().data.Operation.MakeRegister.module_name = v
		broadcast_change()
