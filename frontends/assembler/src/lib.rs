pub mod parser;

use intuicio_core::{IntuicioVersion, crate_version, script::BytesContentParser};
use intuicio_frontend_serde::*;
use std::error::Error;

pub type AsmScript = SerdeScript;
pub type AsmLiteral = SerdeLiteral;
pub type AsmExpression = SerdeExpression;
pub type AsmOperation = SerdeOperation;
pub type AsmFunctionParameter = SerdeFunctionParameter;
pub type AsmFunction = SerdeFunction;
pub type AsmStructField = SerdeStructField;
pub type AsmStruct = SerdeStruct;
pub type AsmEnumVariant = SerdeEnumVariant;
pub type AsmEnum = SerdeEnum;
pub type AsmModule = SerdeModule;
pub type AsmFile = SerdeFile;
pub type AsmPackage = SerdePackage;
pub type AsmNodeTypeInfo = SerdeNodeTypeInfo;
pub type AsmNodes = SerdeNodes;
pub type CompileAsmNodeGraphVisitor = CompileSerdeNodeGraphVisitor;

pub fn frontend_assembly_version() -> IntuicioVersion {
    crate_version!()
}

pub struct AsmContentParser;

impl BytesContentParser<SerdeFile> for AsmContentParser {
    fn parse(&self, bytes: Vec<u8>) -> Result<SerdeFile, Box<dyn Error>> {
        let content = String::from_utf8(bytes)?;
        Ok(parser::parse(&content)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use intuicio_backend_vm::prelude::*;
    use intuicio_core::prelude::*;

    #[test]
    fn test_frontend_asm() {
        let mut registry = Registry::default().with_basic_types();
        registry.add_function(define_function! {
            registry => mod intrinsics fn add(a: usize, b: usize) -> (result: usize) {
                (a + b,)
            }
        });
        let mut content_provider = FileContentProvider::new("iasm", AsmContentParser);
        AsmPackage::new("../../resources/package.iasm", &mut content_provider)
            .unwrap()
            .compile()
            .install::<VmScope<AsmExpression>>(
                &mut registry,
                None,
                // Some(
                //     PrintDebugger::full()
                //         .basic_printables()
                //         .stack_bytes(false)
                //         .registers_bytes(false)
                //         .into_handle(),
                // ),
            );
        assert!(
            registry
                .find_function(FunctionQuery {
                    name: Some("main".into()),
                    module_name: Some("test".into()),
                    ..Default::default()
                })
                .is_some()
        );
        let mut host = Host::new(Context::new(10240, 10240), RegistryHandle::new(registry));
        let (result,) = host
            .call_function::<(usize,), _>("main", "test", None)
            .unwrap()
            .run(());
        assert_eq!(result, 42);
    }
}
