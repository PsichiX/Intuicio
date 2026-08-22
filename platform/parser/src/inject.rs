//! Referring to a parser by name instead of by value.
use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserRegistry};

/// Short constructors for this module.
pub mod shorthand {
    use super::*;

    /// See [`InjectParser`].
    pub fn inject(id: impl ToString) -> ParserHandle {
        InjectParser::new(id).into_handle()
    }
}

/// Looks a parser up in the [`ParserRegistry`] by id and runs it.
///
/// The lookup happens per parse, not when the grammar is built, so the id
/// may still be empty at that point. That is what makes recursive grammars
/// possible: a rule can inject itself.
///
/// Fails when nothing is registered under the id.
#[derive(Clone)]
pub struct InjectParser(String);

impl InjectParser {
    /// Refers to the parser registered as `id`.
    pub fn new(id: impl ToString) -> Self {
        Self(id.to_string())
    }
}

impl Parser for InjectParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        registry.parse(&self.0, input)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserNoValue, ParserOutput, ParserRegistry,
        inject::InjectParser,
        shorthand::{inject, lit, seq, ws},
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_inject() {
        is_async::<InjectParser>();

        let registry = ParserRegistry::default()
            .with_parser("foo", lit("foo"))
            .with_parser("bar", lit("bar"))
            .with_parser("ws", ws())
            .with_parser("main", seq([inject("foo"), inject("ws"), inject("bar")]));
        let (rest, value) = registry.parse("main", "foo   bar").unwrap();
        assert_eq!(rest, "");
        let value = value.consume::<Vec<ParserOutput>>().ok().unwrap();
        assert_eq!(value.len(), 3);
        for value in value {
            assert!(value.read::<String>().is_some() || value.read::<ParserNoValue>().is_some());
        }
    }
}
