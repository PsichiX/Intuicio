class_name ScriptCallFunctionNode
extends ScriptNode

func _ready():
	title = "Call function"
	var value = data().data.Operation.CallFunction.name if data() else ""
	var edit = add_property_string("Name", false, value)
	edit.connect("changed", self, "_changed_name")
	value = data().data.Operation.CallFunction.module_name if data() && "module_name" in data().data.Operation.CallFunction else ""
	edit = add_property_string("Module name", false, value)
	edit.connect("changed", self, "_changed_module_name")
	value = data().data.Operation.CallFunction.struct_name if data() && "struct_name" in data().data.Operation.CallFunction else ""
	edit = add_property_string("Struct name", false, value)
	edit.connect("changed", self, "_changed_struct_name")
	add_execute_in("In")
	add_execute_out("Out")

func _changed_name(v):
	if data():
		data().data.Operation.CallFunction.name = v
		broadcast_change()

func _changed_module_name(v):
	if data():
		data().data.Operation.CallFunction.module_name = v
		broadcast_change()

func _changed_struct_name(v):
	if data():
		data().data.Operation.CallFunction.struct_name = v
		broadcast_change()
