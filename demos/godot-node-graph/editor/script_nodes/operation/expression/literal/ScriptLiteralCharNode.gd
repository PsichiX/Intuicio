class_name ScriptLiteralCharNode
extends ScriptNode

func _ready():
	title = "Character literal"
	var value = data().data.Operation.Expression.Literal.Char if data() else "@";
	var edit = add_property_string("Value", true, value)
	edit.connect("changed", self, "_changed")
	add_execute_in("In")
	add_execute_out("Out")

func _changed(v):
	if data():
		data().data.Operation.Expression.Literal.Char = v
		broadcast_change()
