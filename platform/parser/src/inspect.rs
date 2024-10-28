use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};
use std::sync::{Arc, RwLock};

pub mod shorthand {
    use super::*;

    pub fn inspect(
        parser: ParserHandle,
        f: impl FnMut(&ParserOutput) + Send + Sync + 'static,
    ) -> ParserHandle {
        InspectParser::new(parser, f).into_handle()
    }
}

pub struct InspectParser {
    parser: ParserHandle,
    #[allow(clippy::type_complexity)]
    closure: Arc<RwLock<dyn FnMut(&ParserOutput) + Send + Sync>>,
}

impl InspectParser {
    pub fn new(parser: ParserHandle, f: impl FnMut(&ParserOutput) + Send + Sync + 'static) -> Self {
        Self {
            parser,
            closure: Arc::new(RwLock::new(f)),
        }
    }
}

impl Parser for InspectParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let (input, result) = self.parser.parse(registry, input)?;
        match self.closure.write() {
            Ok(mut closure) => {
                (closure)(&result);
            }
            Err(_) => return Err("InspectParser cannot access closure mutably".into()),
        }
        Ok((input, result))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        inspect::InspectParser,
        shorthand::{inspect, lit},
        ParserRegistry,
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_inspect() {
        is_async::<InspectParser>();

        let registry = ParserRegistry::default();
        let sentence = inspect(lit("foo"), |output| {
            assert!(output.is::<String>());
        });
        let (rest, result) = sentence.parse(&registry, "foo").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.read::<String>().unwrap().as_str(), "foo");
    }
}
