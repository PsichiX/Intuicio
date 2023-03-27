use crate::{
    function::FunctionQuery,
    script::{
        ScriptExpression, ScriptFunction, ScriptFunctionSignature, ScriptOperation, ScriptStruct,
        ScriptStructField,
    },
    struct_type::StructQuery,
};
use std::fmt::{Error, Write};

pub trait ScriptNativizer<SE: ScriptExpression> {
    fn nativize_struct(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptStruct,
    ) -> Result<(), Error> {
        self.nativize_struct_begin(output, input)?;
        for field in &input.fields {
            self.nativize_struct_field(output, field)?;
        }
        self.nativize_struct_end(output, input)
    }

    fn nativize_struct_begin(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptStruct,
    ) -> Result<(), Error>;

    fn nativize_struct_end(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptStruct,
    ) -> Result<(), Error>;

    fn nativize_struct_field(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptStructField,
    ) -> Result<(), Error>;

    fn nativize_function(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptFunction<SE>,
    ) -> Result<(), Error> {
        self.nativize_function_begin(output, input)?;
        self.nativize_function_signature(output, &input.signature)?;
        self.nativize_function_body_begin(output, input)?;
        self.nativize_script(output, &input.script)?;
        self.nativize_function_body_end(output, input)?;
        self.nativize_function_end(output, input)
    }

    #[allow(unused_variables)]
    fn nativize_function_begin(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptFunction<SE>,
    ) -> Result<(), Error> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn nativize_function_end(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptFunction<SE>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn nativize_function_signature(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptFunctionSignature,
    ) -> Result<(), Error>;

    #[allow(unused_variables)]
    fn nativize_function_body_begin(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptFunction<SE>,
    ) -> Result<(), Error> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn nativize_function_body_end(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptFunction<SE>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn nativize_script(
        &mut self,
        output: &mut dyn Write,
        input: &[ScriptOperation<SE>],
    ) -> Result<(), Error> {
        self.nativize_script_begin(output, input)?;
        for operation in input {
            self.nativize_operation(output, operation)?;
        }
        self.nativize_script_end(output, input)
    }

    fn nativize_script_begin(
        &mut self,
        output: &mut dyn Write,
        input: &[ScriptOperation<SE>],
    ) -> Result<(), Error>;

    fn nativize_script_end(
        &mut self,
        output: &mut dyn Write,
        input: &[ScriptOperation<SE>],
    ) -> Result<(), Error>;

    fn nativize_operation(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptOperation<SE>,
    ) -> Result<(), Error> {
        self.nativize_operation_begin(output, input)?;
        match input {
            ScriptOperation::None => {}
            ScriptOperation::Expression { expression } => {
                self.nativize_operation_expression(output, expression)?;
            }
            ScriptOperation::DefineRegister { query } => {
                self.nativize_operation_define_register(output, query)?;
            }
            ScriptOperation::DropRegister { index } => {
                self.nativize_operation_drop_register(output, *index)?;
            }
            ScriptOperation::PushFromRegister { index } => {
                self.nativize_operation_push_from_register(output, *index)?;
            }
            ScriptOperation::PopToRegister { index } => {
                self.nativize_operation_pop_to_register(output, *index)?;
            }
            ScriptOperation::MoveRegister { from, to } => {
                self.nativize_operation_move_register(output, *from, *to)?;
            }
            ScriptOperation::CallFunction { query } => {
                self.nativize_operation_call_function(output, query)?;
            }
            ScriptOperation::BranchScope {
                scope_success,
                scope_failure,
            } => {
                self.nativize_operation_branch_scope(
                    output,
                    scope_success,
                    scope_failure.as_ref().map(|scope| scope.as_slice()),
                )?;
            }
            ScriptOperation::LoopScope { scope } => {
                self.nativize_operation_loop_scope(output, scope)?;
            }
            ScriptOperation::PushScope { scope } => {
                self.nativize_operation_push_scope(output, scope)?;
            }
            ScriptOperation::PopScope => {
                self.nativize_operation_pop_scope(output)?;
            }
            ScriptOperation::ContinueScopeConditionally => {
                self.nativize_operation_continue_scope_conditionally(output)?;
            }
        }
        self.nativize_operation_end(output, input)
    }

    #[allow(unused_variables)]
    fn nativize_operation_begin(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptOperation<SE>,
    ) -> Result<(), Error> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn nativize_operation_end(
        &mut self,
        output: &mut dyn Write,
        input: &ScriptOperation<SE>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn nativize_operation_expression(
        &mut self,
        output: &mut dyn Write,
        input: &SE,
    ) -> Result<(), Error>;

    fn nativize_operation_define_register(
        &mut self,
        output: &mut dyn Write,
        query: &StructQuery,
    ) -> Result<(), Error>;

    fn nativize_operation_drop_register(
        &mut self,
        output: &mut dyn Write,
        index: usize,
    ) -> Result<(), Error>;

    fn nativize_operation_push_from_register(
        &mut self,
        output: &mut dyn Write,
        index: usize,
    ) -> Result<(), Error>;

    fn nativize_operation_pop_to_register(
        &mut self,
        output: &mut dyn Write,
        index: usize,
    ) -> Result<(), Error>;

    fn nativize_operation_move_register(
        &mut self,
        output: &mut dyn Write,
        from: usize,
        to: usize,
    ) -> Result<(), Error>;

    fn nativize_operation_call_function(
        &mut self,
        output: &mut dyn Write,
        query: &FunctionQuery,
    ) -> Result<(), Error>;

    fn nativize_operation_branch_scope(
        &mut self,
        output: &mut dyn Write,
        scope_success: &[ScriptOperation<SE>],
        scope_failure: Option<&[ScriptOperation<SE>]>,
    ) -> Result<(), Error>;

    fn nativize_operation_loop_scope(
        &mut self,
        output: &mut dyn Write,
        scope: &[ScriptOperation<SE>],
    ) -> Result<(), Error>;

    fn nativize_operation_push_scope(
        &mut self,
        output: &mut dyn Write,
        scope: &[ScriptOperation<SE>],
    ) -> Result<(), Error>;

    fn nativize_operation_pop_scope(&mut self, output: &mut dyn Write) -> Result<(), Error>;

    fn nativize_operation_continue_scope_conditionally(
        &mut self,
        output: &mut dyn Write,
    ) -> Result<(), Error>;
}
