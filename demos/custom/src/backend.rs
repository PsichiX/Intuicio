use intuicio_core::{
    context::Context,
    function::FunctionBody,
    registry::Registry,
    script::{ScriptExpression, ScriptFunctionGenerator, ScriptHandle, ScriptOperation},
};

pub struct CustomScope<'a, SE: ScriptExpression> {
    handle: ScriptHandle<'a, SE>,
    position: usize,
}

impl<'a, SE: ScriptExpression> CustomScope<'a, SE> {
    pub fn new(handle: ScriptHandle<'a, SE>) -> Self {
        Self {
            handle,
            position: 0,
        }
    }

    pub fn run(&mut self, context: &mut Context, registry: &Registry) {
        while let Some(operation) = self.handle.get(self.position) {
            match operation {
                ScriptOperation::None => {
                    self.position += 1;
                }
                ScriptOperation::Expression { expression } => {
                    expression.evaluate(context, registry);
                    self.position += 1;
                }
                ScriptOperation::CallFunction { query } => {
                    let handle = registry
                        .functions()
                        .find(|handle| query.is_valid(handle.signature()))
                        .unwrap_or_else(|| {
                            panic!("Could not call non-existent function: {query:#?}")
                        });
                    handle.invoke(context, registry);
                    self.position += 1;
                }
                _ => unreachable!("Trying to perform unsupported operation!"),
            }
        }
    }
}

impl<SE: ScriptExpression + 'static> ScriptFunctionGenerator<SE> for CustomScope<'static, SE> {
    type Input = ();
    type Output = ();

    fn generate_function_body(
        script: ScriptHandle<'static, SE>,
        ignore: Self::Input,
    ) -> Option<(FunctionBody, Self::Output)> {
        Some((
            FunctionBody::closure(move |context, registry| {
                Self::new(script.clone()).run(context, registry);
            }),
            ignore,
        ))
    }
}
