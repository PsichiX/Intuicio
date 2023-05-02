class_name ScriptLiteralSignedNode
extends ScriptNode

export(String) var variant = ""

func _ready():
	title = "Signed literal"
	var value = data().data.Operation.Expression.Literal[variant] if data() else 0;
	var edit = add_property_int("Value", true, value)
	edit.connect("changed", self, "_changed")
	add_execute_in("In")
	add_execute_out("Out")

func _changed(v):
	if data():
		data().data.Operation.Expression.Literal[variant] = v
		broadcast_change()
