class_name ScriptLiteralBoolNode
extends ScriptNode

func _ready():
	title = "Boolean literal"
	var value = data().data.Operation.Expression.Literal.Bool if data() else false;
	var edit = add_property_bool("Value", value)
	edit.connect("toggled", self, "_toggled")
	add_execute_in("In")
	add_execute_out("Out")

func _toggled(v):
	if data():
		data().data.Operation.Expression.Literal.Bool = v
		broadcast_change()
