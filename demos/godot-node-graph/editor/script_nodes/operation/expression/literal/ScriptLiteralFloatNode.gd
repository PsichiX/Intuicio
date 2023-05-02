class_name ScriptLiteralFloatNode
extends ScriptNode

export(String) var variant = ""

func _ready():
	title = "Float literal"
	var value = data().data.Operation.Expression.Literal[variant] if data() else 0.0;
	var edit = add_property_float("Value", value)
	edit.connect("changed", self, "_changed")
	add_execute_in("In")
	add_execute_out("Out")

func _changed(v):
	if data():
		data().data.Operation.Expression.Literal[variant] = v
		broadcast_change()
