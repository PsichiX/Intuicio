class_name ScriptLiteralStringNode
extends ScriptNode

func _ready():
	title = "String literal"
	var value = data().data.Operation.Expression.Literal.String if data() else "";
	var edit = add_property_string("Value", false, value)
	edit.connect("changed", self, "_changed")
	add_execute_in("In")
	add_execute_out("Out")

func _changed(v):
	if data():
		data().data.Operation.Expression.Literal.String = v
		broadcast_change()
