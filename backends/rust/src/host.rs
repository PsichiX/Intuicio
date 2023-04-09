use intuicio_core::{
    context::Context,
    function::{
        Function, FunctionBody, FunctionParameter, FunctionQuery, FunctionQueryParameter,
        FunctionSignature,
    },
    host::Host,
    nativizer::ScriptNativizer,
    registry::Registry,
    script::{
        ScriptExpression, ScriptFunction, ScriptFunctionSignature, ScriptOperation, ScriptStruct,
        ScriptStructField,
    },
    struct_type::{StructFieldQuery, StructQuery},
    Visibility,
};
use std::fmt::{Error, Write};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ScopeKind {
    Normal,
    Branch,
    Loop,
}

pub trait RustHostExpressionNativizer<SE: ScriptExpression> {
    fn nativize_expression(&mut self, output: &mut dyn Write, input: &SE) -> Result<(), Error>;
}

impl<SE: ScriptExpression> RustHostExpressionNativizer<SE> for () {
    fn nativize_expression(&mut self, _: &mut dyn Write, _: &SE) -> Result<(), Error> {
        Ok(())
    }
}

pub struct RustHostNativizer<'a, SE: ScriptExpression> {
    pub tab_size: usize,
    indent: usize,
    scope_kind: Vec<ScopeKind>,
    registry: &'a Registry,
    expression_nativizer: Box<dyn RustHostExpressionNativizer<SE>>,
}

impl<'a, SE: ScriptExpression> RustHostNativizer<'a, SE> {
    pub fn new(
        registry: &'a Registry,
        expression_nativizer: impl RustHostExpressionNativizer<SE> + 'static,
    ) -> Self {
        Self {
            tab_size: 4,
            indent: 0,
            scope_kind: vec![],
            registry,
            expression_nativizer: Box::new(expression_nativizer),
        }
    }

    pub fn with_tab_size(mut self, size: usize) -> Self {
        self.tab_size = size;
        self
    }

    pub fn push_indent(&mut self) {
        self.indent += 1;
    }

    pub fn pop_indent(&mut self) {
        if self.indent > 0 {
            self.indent -= 1;
        }
    }

    pub fn pad(&mut self, output: &mut dyn Write) -> Result<(), Error> {
        for _ in 0..(self.indent * self.tab_size) {
            output.write_char(' ')?;
        }
        Ok(())
    }

    pub fn write_visibility(
        &mut self,
        output: &mut dyn Write,
        input: Visibility,
    ) -> Result<(), Error> {
        match input {
            Visibility::Private => {}
            Visibility::Module => {
                write!(output, "pub(crate) ")?;
            }
            Visibility::Public => {
                write!(output, "pub ")?;
            }
        }
        Ok(())
    }

    pub fn write_struct_field_query(
        &mut self,
        output: &mut dyn Write,
        input: &StructFieldQuery,
    ) -> Result<(), Error> {
        writeln!(output, "{} {{", std::any::type_name::<StructFieldQuery>())?;
        self.push_indent();
        self.pad(output)?;
        if let Some(name) = input.name.as_ref() {
            writeln!(output, r#"name: Some("{}".into()),"#, name.as_ref())?;
        } else {
            writeln!(output, "name: None,")?;
        }
        self.pad(output)?;
        if let Some(struct_query) = input.struct_query.as_ref() {
            write!(output, "struct_query: Some(")?;
            self.write_struct_query(output, struct_query)?;
            writeln!(output, "),")?;
        } else {
            writeln!(output, "struct_query: None,")?;
        }
        self.pad(output)?;
        writeln!(
            output,
            "visibility: {}::{:?},",
            std::any::type_name::<Visibility>(),
            input.visibility
        )?;
        self.pop_indent();
        self.pad(output)?;
        write!(output, "}}")
    }

    pub fn write_struct_query(
        &mut self,
        output: &mut dyn Write,
        input: &StructQuery,
    ) -> Result<(), Error> {
        writeln!(output, "{} {{", std::any::type_name::<StructQuery>())?;
        self.push_indent();
        self.pad(output)?;
        if let Some(name) = input.name.as_ref() {
            writeln!(output, r#"name: Some("{}".into()),"#, name.as_ref())?;
        } else {
            writeln!(output, "name: None,")?;
        }
        self.pad(output)?;
        writeln!(output, "type_hash: None,")?;
        self.pad(output)?;
        if let Some(module_name) = input.module_name.as_ref() {
            writeln!(
                output,
                r#"module_name: Some("{}".into()),"#,
                module_name.as_ref()
            )?;
        } else {
            writeln!(output, "module_name: None,")?;
        }
        self.pad(output)?;
        if let Some(visibility) = input.visibility {
            writeln!(
                output,
                "visibility: Some({}::{:?}),",
                std::any::type_name::<Visibility>(),
                visibility
            )?;
        } else {
            writeln!(output, "visibility: None,")?;
        }
        self.pad(output)?;
        writeln!(output, "fields: [")?;
        self.push_indent();
        for field in input.fields.as_ref() {
            self.pad(output)?;
            self.write_struct_field_query(output, field)?;
            writeln!(output, ",")?;
        }
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "].as_slice().into(),")?;
        self.pop_indent();
        self.pad(output)?;
        write!(output, "}}")
    }

    pub fn write_function_parameter_query(
        &mut self,
        output: &mut dyn Write,
        input: &FunctionQueryParameter,
    ) -> Result<(), Error> {
        writeln!(
            output,
            "{} {{",
            std::any::type_name::<FunctionQueryParameter>()
        )?;
        self.push_indent();
        self.pad(output)?;
        if let Some(name) = input.name.as_ref() {
            writeln!(output, r#"name: Some("{}".into()),"#, name.as_ref())?;
        } else {
            writeln!(output, "name: None,")?;
        }
        self.pad(output)?;
        if let Some(struct_query) = input.struct_query.as_ref() {
            write!(output, "struct_query: Some(")?;
            self.write_struct_query(output, struct_query)?;
            writeln!(output, "),")?;
        } else {
            writeln!(output, "struct_query: None,")?;
        }
        self.pop_indent();
        self.pad(output)?;
        write!(output, "}}")
    }

    pub fn write_function_query(
        &mut self,
        output: &mut dyn Write,
        input: &FunctionQuery,
    ) -> Result<(), Error> {
        writeln!(output, "{} {{", std::any::type_name::<FunctionQuery>())?;
        self.push_indent();
        self.pad(output)?;
        if let Some(name) = input.name.as_ref() {
            writeln!(output, r#"name: Some("{}".into()),"#, name.as_ref())?;
        } else {
            writeln!(output, "name: None,")?;
        }
        self.pad(output)?;
        if let Some(struct_query) = input.struct_query.as_ref() {
            write!(output, "Some(")?;
            self.write_struct_query(output, struct_query)?;
            writeln!(output, "),")?;
        } else {
            writeln!(output, "struct_query: None,")?;
        }
        self.pad(output)?;
        if let Some(module_name) = input.module_name.as_ref() {
            writeln!(
                output,
                r#"module_name: Some("{}".into()),"#,
                module_name.as_ref()
            )?;
        } else {
            writeln!(output, "module_name: None,")?;
        }
        self.pad(output)?;
        if let Some(visibility) = input.visibility {
            writeln!(
                output,
                "visibility: Some({}::{:?}),",
                std::any::type_name::<Visibility>(),
                visibility
            )?;
        } else {
            writeln!(output, "visibility: None,")?;
        }
        self.pad(output)?;
        writeln!(output, "inputs: [")?;
        self.push_indent();
        for parameter in input.inputs.as_ref() {
            self.pad(output)?;
            self.write_function_parameter_query(output, parameter)?;
            writeln!(output, ",")?;
        }
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "].as_slice().into(),")?;
        self.pad(output)?;
        writeln!(output, "outputs: [")?;
        self.push_indent();
        for parameter in input.outputs.as_ref() {
            self.pad(output)?;
            self.write_function_parameter_query(output, parameter)?;
            writeln!(output, ",")?;
        }
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "].as_slice().into(),")?;
        self.pop_indent();
        self.pad(output)?;
        write!(output, "}}")
    }

    fn write_function(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptFunction<SE>,
    ) -> Result<(), Error> {
        self.pad(output)?;
        self.write_visibility(output, input.signature.visibility)?;
        write!(output, "fn {}(", input.signature.name,)?;
        for parameter in &input.signature.inputs {
            write!(
                output,
                "{}: {},",
                parameter.name,
                self.registry
                    .structs()
                    .find(|struct_type| parameter.struct_query.is_valid(struct_type))
                    .unwrap()
                    .type_name()
            )?;
        }
        write!(output, ") -> (")?;
        for parameter in &input.signature.outputs {
            write!(
                output,
                "{},",
                self.registry
                    .structs()
                    .find(|struct_type| parameter.struct_query.is_valid(struct_type))
                    .unwrap()
                    .type_name()
            )?;
        }
        writeln!(output, ") {{")?;
        self.push_indent();
        self.pad(output)?;
        writeln!(
            output,
            "{}::with_global(move |mut host| {{",
            std::any::type_name::<Host>()
        )?;
        self.push_indent();
        self.pad(output)?;
        writeln!(
            output,
            "let (mut __context__, __registry__) = host.context_and_registry();"
        )?;
        for parameter in input.signature.inputs.iter().rev() {
            self.pad(output)?;
            writeln!(output, "__context__.stack().push({});", parameter.name)?;
        }
        self.pad(output)?;
        writeln!(
            output,
            "{}::intuicio_function(__context__, __registry__);",
            input.signature.name
        )?;
        self.pad(output)?;
        write!(output, "(")?;
        for parameter in &input.signature.outputs {
            write!(
                output,
                "__context__.stack().pop::<{}>().unwrap(),",
                self.registry
                    .structs()
                    .find(|struct_type| parameter.struct_query.is_valid(struct_type))
                    .unwrap()
                    .type_name()
            )?;
        }
        writeln!(output, ")")?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(
            output,
            r#"}}).expect("There is no global host for current thread to run function: `{:?}`")"#,
            input.signature
        )?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")?;
        writeln!(output)?;
        self.pad(output)?;
        writeln!(output, "pub mod {} {{", input.signature.name)?;
        self.push_indent();
        self.pad(output)?;
        writeln!(
            output,
            "pub fn define_signature(registry: &{}) -> {} {{",
            std::any::type_name::<Registry>(),
            std::any::type_name::<FunctionSignature>()
        )?;
        self.push_indent();
        self.pad(output)?;
        writeln!(
            output,
            r#"let mut result = {}::new("{}");"#,
            std::any::type_name::<FunctionSignature>(),
            input.signature.name
        )?;
        if let Some(module_name) = input.signature.module_name.as_ref() {
            self.pad(output)?;
            writeln!(
                output,
                r#"result.module_name = Some("{}".to_owned());"#,
                module_name
            )?;
        }
        self.pad(output)?;
        writeln!(
            output,
            "result.visibility = {}::{:?};",
            std::any::type_name::<Visibility>(),
            input.signature.visibility
        )?;
        for parameter in &input.signature.inputs {
            self.pad(output)?;
            writeln!(
                output,
                r#"result.inputs.push({}::new("{}", registry.find_struct({}::of::<{}>()).unwrap()));"#,
                std::any::type_name::<FunctionParameter>(),
                parameter.name,
                std::any::type_name::<StructQuery>(),
                self.registry
                    .structs()
                    .find(|struct_type| parameter.struct_query.is_valid(struct_type))
                    .unwrap()
                    .type_name()
            )?;
        }
        for parameter in &input.signature.outputs {
            self.pad(output)?;
            writeln!(
                output,
                r#"result.outputs.push({}::new("{}", registry.find_struct({}::of::<{}>()).unwrap()));"#,
                std::any::type_name::<FunctionParameter>(),
                parameter.name,
                std::any::type_name::<StructQuery>(),
                self.registry
                    .structs()
                    .find(|struct_type| parameter.struct_query.is_valid(struct_type))
                    .unwrap()
                    .type_name()
            )?;
        }
        self.pad(output)?;
        writeln!(output, "result")?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")?;
        writeln!(output)?;
        self.pad(output)?;
        writeln!(
            output,
            "pub fn define_function(registry: &{}) -> {} {{",
            std::any::type_name::<Registry>(),
            std::any::type_name::<Function>()
        )?;
        self.push_indent();
        self.pad(output)?;
        writeln!(
            output,
            "{}::new(define_signature(registry), {}::pointer(intuicio_function))",
            std::any::type_name::<Function>(),
            std::any::type_name::<FunctionBody>()
        )?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")?;
        writeln!(output)
    }
}

impl<'a, SE: ScriptExpression> ScriptNativizer<SE> for RustHostNativizer<'a, SE> {
    fn nativize_struct_begin(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptStruct,
    ) -> Result<(), Error> {
        self.pad(output)?;
        writeln!(output, "#[derive(IntuicioScript)]")?;
        if let Some(module_name) = input.module_name.as_ref() {
            self.pad(output)?;
            writeln!(output, r#"#[intuicio(module_name = "{}")]"#, module_name)?;
        }
        self.pad(output)?;
        self.write_visibility(output, input.visibility)?;
        writeln!(output, "struct {} {{", input.name)?;
        self.push_indent();
        Ok(())
    }

    fn nativize_struct_end(
        &mut self,
        output: &mut dyn Write,
        _: &ScriptStruct,
    ) -> Result<(), Error> {
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")
    }

    fn nativize_struct_field(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptStructField,
    ) -> Result<(), Error> {
        self.pad(output)?;
        self.write_visibility(output, input.visibility)?;
        writeln!(
            output,
            "{}: {},",
            input.name,
            self.registry
                .structs()
                .find(|struct_type| input.struct_query.is_valid(struct_type))
                .unwrap()
                .type_name()
        )
    }

    fn nativize_function_begin(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptFunction<SE>,
    ) -> Result<(), Error> {
        self.write_function(output, input)
        // TODO: for now we nativize struct methods as regular functions.
        // the only difference between them is that standard puts method
        // generated code in struct implementations.
        // if input.signature.struct_query.is_some() {
        //     self.write_method(output, input)
        // } else {
        //     self.write_function(output, input)
        // }
    }

    fn nativize_function_end(
        &mut self,
        output: &mut dyn Write,
        _: &ScriptFunction<SE>,
    ) -> Result<(), Error> {
        writeln!(output)?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")?;
        Ok(())
    }

    fn nativize_function_signature(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptFunctionSignature,
    ) -> Result<(), Error> {
        self.pad(output)?;
        self.write_visibility(output, input.visibility)?;
        write!(
            output,
            "fn intuicio_function(context: &mut {}, registry: &{})",
            std::any::type_name::<Context>(),
            std::any::type_name::<Registry>()
        )
    }

    fn nativize_function_body_begin(
        &mut self,
        output: &mut dyn Write,
        _: &ScriptFunction<SE>,
    ) -> Result<(), Error> {
        write!(output, " ")
    }

    fn nativize_script_begin(
        &mut self,
        output: &mut dyn Write,
        _: &[ScriptOperation<SE>],
    ) -> Result<(), Error> {
        if let Some(kind) = self.scope_kind.last() {
            if *kind != ScopeKind::Loop {
                write!(output, "'scope{}: ", self.scope_kind.len())?;
            }
        }
        writeln!(output, "{{")?;
        self.push_indent();
        Ok(())
    }

    fn nativize_script_end(
        &mut self,
        output: &mut dyn Write,
        _: &[ScriptOperation<SE>],
    ) -> Result<(), Error> {
        self.pop_indent();
        self.pad(output)?;
        write!(output, "}}")
    }

    fn nativize_operation_expression(
        &mut self,
        output: &mut dyn Write,
        input: &SE,
    ) -> Result<(), Error> {
        self.expression_nativizer.nativize_expression(output, input)
    }

    fn nativize_operation_define_register(
        &mut self,
        output: &mut dyn Write,
        query: &StructQuery,
    ) -> Result<(), Error> {
        self.pad(output)?;
        writeln!(output, "{{")?;
        self.push_indent();
        self.pad(output)?;
        write!(output, "let query = ")?;
        self.write_struct_query(output, query)?;
        writeln!(output, ";")?;
        self.pad(output)?;
        writeln!(
            output,
            "let handle = registry.structs().find(|handle| query.is_valid(handle)).unwrap();"
        )?;
        self.pad(output)?;
        writeln!(
            output,
            "unsafe {{ context.registers().push_register_raw(handle.type_hash(), *handle.layout()) }};"
        )?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")
    }

    fn nativize_operation_drop_register(
        &mut self,
        output: &mut dyn Write,
        index: usize,
    ) -> Result<(), Error> {
        self.pad(output)?;
        writeln!(output, "{{")?;
        self.push_indent();
        self.pad(output)?;
        writeln!(
            output,
            "let index = context.absolute_register_index({});",
            index
        )?;
        self.pad(output)?;
        writeln!(
            output,
            "context.registers().access_register(index).unwrap().free();"
        )?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")
    }

    fn nativize_operation_push_from_register(
        &mut self,
        output: &mut dyn Write,
        index: usize,
    ) -> Result<(), Error> {
        self.pad(output)?;
        writeln!(output, "{{")?;
        self.push_indent();
        self.pad(output)?;
        writeln!(
            output,
            "let index = context.absolute_register_index({});",
            index
        )?;
        self.pad(output)?;
        writeln!(
            output,
            "let (stack, registers) = context.stack_and_registers();"
        )?;
        self.pad(output)?;
        writeln!(
            output,
            "let mut register = registers.access_register(index).unwrap();"
        )?;
        self.pad(output)?;
        writeln!(
            output,
            r#"if !stack.push_from_register(&mut register) {{ panic!("Could not push data from register: {}"); }}"#,
            index
        )?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")
    }

    fn nativize_operation_pop_to_register(
        &mut self,
        output: &mut dyn Write,
        index: usize,
    ) -> Result<(), Error> {
        self.pad(output)?;
        writeln!(output, "{{")?;
        self.push_indent();
        self.pad(output)?;
        writeln!(
            output,
            "let index = context.absolute_register_index({});",
            index
        )?;
        self.pad(output)?;
        writeln!(
            output,
            "let (stack, registers) = context.stack_and_registers();"
        )?;
        self.pad(output)?;
        writeln!(
            output,
            "let mut register = registers.access_register(index).unwrap();"
        )?;
        self.pad(output)?;
        writeln!(
            output,
            r#"if !stack.pop_to_register(&mut register) {{ panic!("Could not pop data to register: {}"); }}"#,
            index
        )?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")
    }

    fn nativize_operation_move_register(
        &mut self,
        output: &mut dyn Write,
        from: usize,
        to: usize,
    ) -> Result<(), Error> {
        self.pad(output)?;
        writeln!(output, "{{")?;
        self.push_indent();
        self.pad(output)?;
        writeln!(
            output,
            "let from = context.absolute_register_index({});",
            from
        )?;
        self.pad(output)?;
        writeln!(output, "let to = context.absolute_register_index({});", to)?;
        self.pad(output)?;
        writeln!(
            output,
            "let (mut source, mut target) = context.registers().access_registers_pair(from, to).unwrap();"
        )?;
        self.pad(output)?;
        writeln!(output, "source.move_to(&mut target);")?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")
    }

    fn nativize_operation_call_function(
        &mut self,
        output: &mut dyn Write,
        query: &FunctionQuery,
    ) -> Result<(), Error> {
        self.pad(output)?;
        writeln!(output, "{{")?;
        self.push_indent();
        self.pad(output)?;
        write!(output, "let query = ")?;
        self.write_function_query(output, query)?;
        writeln!(output, ";")?;
        self.pad(output)?;
        writeln!(output, "registry.functions().find(|handle| query.is_valid(handle.signature())).unwrap().invoke(context, registry);")?;
        self.pop_indent();
        self.pad(output)?;
        writeln!(output, "}}")
    }

    fn nativize_operation_branch_scope(
        &mut self,
        output: &mut dyn Write,
        scope_success: &[ScriptOperation<SE>],
        scope_failure: Option<&[ScriptOperation<SE>]>,
    ) -> Result<(), Error> {
        self.pad(output)?;
        writeln!(output, "if context.stack().pop::<bool>().unwrap() {{")?;
        self.push_indent();
        self.pad(output)?;
        self.scope_kind.push(ScopeKind::Branch);
        self.nativize_script(output, scope_success)?;
        self.scope_kind.pop();
        self.pop_indent();
        writeln!(output)?;
        self.pad(output)?;
        write!(output, "}}")?;
        if let Some(scope_failure) = scope_failure.as_ref() {
            writeln!(output, " else {{")?;
            self.push_indent();
            self.pad(output)?;
            self.scope_kind.push(ScopeKind::Branch);
            self.nativize_script(output, scope_failure)?;
            self.scope_kind.pop();
            self.pop_indent();
            writeln!(output)?;
            self.pad(output)?;
            write!(output, "}}")?;
        }
        writeln!(output)
    }

    fn nativize_operation_loop_scope(
        &mut self,
        output: &mut dyn Write,
        scope: &[ScriptOperation<SE>],
    ) -> Result<(), Error> {
        self.pad(output)?;
        write!(output, "loop ")?;
        self.scope_kind.push(ScopeKind::Loop);
        self.nativize_script(output, scope)?;
        self.scope_kind.pop();
        writeln!(output)?;
        Ok(())
    }

    fn nativize_operation_push_scope(
        &mut self,
        output: &mut dyn Write,
        scope: &[ScriptOperation<SE>],
    ) -> Result<(), Error> {
        self.pad(output)?;
        self.scope_kind.push(ScopeKind::Normal);
        self.nativize_script(output, scope)?;
        self.scope_kind.pop();
        writeln!(output)?;
        Ok(())
    }

    fn nativize_operation_pop_scope(&mut self, output: &mut dyn Write) -> Result<(), Error> {
        self.pad(output)?;
        match self.scope_kind.last() {
            Some(ScopeKind::Normal) | Some(ScopeKind::Branch) => {
                writeln!(output, "break 'scope{};", self.scope_kind.len())
            }
            Some(ScopeKind::Loop) => {
                writeln!(output, "break;")
            }
            None => {
                writeln!(output, "return;")
            }
        }
    }

    fn nativize_operation_continue_scope_conditionally(
        &mut self,
        output: &mut dyn Write,
    ) -> Result<(), Error> {
        self.pad(output)?;
        write!(output, "if !context.stack().pop::<bool>().unwrap() {{ ")?;
        match self.scope_kind.last() {
            Some(ScopeKind::Normal) | Some(ScopeKind::Branch) => {
                write!(output, "break 'scope{};", self.scope_kind.len())?;
            }
            Some(ScopeKind::Loop) => {
                write!(output, "break;")?;
            }
            None => {
                write!(output, "return;")?;
            }
        }
        writeln!(output, " }}")
    }
}

#[cfg(test)]
mod tests {
    use super::RustHostNativizer;
    use intuicio_core::{
        function::FunctionQuery,
        nativizer::ScriptNativizer,
        registry::Registry,
        script::{
            ScriptBuilder, ScriptFunction, ScriptFunctionParameter, ScriptFunctionSignature,
            ScriptStruct, ScriptStructField,
        },
        struct_type::StructQuery,
        Visibility,
    };
    use intuicio_data::type_hash::TypeHash;

    #[test]
    fn test_rust_host_nativization() {
        let mut registry = Registry::default().with_basic_types();

        let struct_type = ScriptStruct {
            name: "Foo".to_owned(),
            module_name: Some("test".to_owned()),
            visibility: Visibility::Public,
            fields: vec![
                ScriptStructField {
                    name: "a".to_owned(),
                    visibility: Visibility::Public,
                    struct_query: StructQuery {
                        type_hash: Some(TypeHash::of::<bool>()),
                        ..Default::default()
                    },
                },
                ScriptStructField {
                    name: "b".to_owned(),
                    visibility: Visibility::Public,
                    struct_query: StructQuery {
                        type_hash: Some(TypeHash::of::<usize>()),
                        ..Default::default()
                    },
                },
                ScriptStructField {
                    name: "c".to_owned(),
                    visibility: Visibility::Public,
                    struct_query: StructQuery {
                        type_hash: Some(TypeHash::of::<f32>()),
                        ..Default::default()
                    },
                },
            ],
        };
        struct_type.install(&mut registry);
        let mut buffer = String::new();
        RustHostNativizer::<()>::new(&registry, ())
            .nativize_struct(&mut buffer, &struct_type)
            .unwrap();
        println!("{}", buffer);

        let function = ScriptFunction::<()> {
            signature: ScriptFunctionSignature {
                name: "foo".to_owned(),
                module_name: Some("test".to_owned()),
                struct_query: None,
                visibility: Visibility::Public,
                inputs: vec![
                    ScriptFunctionParameter {
                        name: "a".to_owned(),
                        struct_query: StructQuery {
                            type_hash: Some(TypeHash::of::<bool>()),
                            ..Default::default()
                        },
                    },
                    ScriptFunctionParameter {
                        name: "b".to_owned(),
                        struct_query: StructQuery {
                            type_hash: Some(TypeHash::of::<usize>()),
                            ..Default::default()
                        },
                    },
                ],
                outputs: vec![ScriptFunctionParameter {
                    name: "c".to_owned(),
                    struct_query: StructQuery {
                        type_hash: Some(TypeHash::of::<f32>()),
                        ..Default::default()
                    },
                }],
            },
            script: ScriptBuilder::default()
                .define_register(StructQuery {
                    name: Some("i32".into()),
                    ..Default::default()
                })
                .drop_register(0)
                .push_from_register(0)
                .pop_to_register(0)
                .move_register(0, 1)
                .call_function(FunctionQuery {
                    name: Some("foo".into()),
                    ..Default::default()
                })
                .branch_scope(
                    ScriptBuilder::default()
                        .pop_to_register(0)
                        .move_register(0, 1)
                        .continue_scope_conditionally()
                        .pop_scope()
                        .build(),
                    Some(
                        ScriptBuilder::default()
                            .pop_to_register(0)
                            .move_register(0, 1)
                            .continue_scope_conditionally()
                            .pop_scope()
                            .build(),
                    ),
                )
                .loop_scope(
                    ScriptBuilder::default()
                        .pop_to_register(0)
                        .move_register(0, 1)
                        .continue_scope_conditionally()
                        .pop_scope()
                        .build(),
                )
                .push_scope(
                    ScriptBuilder::default()
                        .pop_to_register(0)
                        .move_register(0, 1)
                        .continue_scope_conditionally()
                        .pop_scope()
                        .build(),
                )
                .continue_scope_conditionally()
                .pop_scope()
                .build(),
        };
        let mut buffer = String::new();
        RustHostNativizer::new(&registry, ())
            .nativize_function(&mut buffer, &function)
            .unwrap();
        println!("{}", buffer);
    }
}
