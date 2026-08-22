//! Making a parser allowed to fail.
use crate::{
    ParseResult, Parser, ParserExt, ParserHandle, ParserNoValue, ParserOutput, ParserRegistry,
};

/// Short constructors for this module.
pub mod shorthand {
    use super::*;

    /// See [`OptionalParser`].
    pub fn opt(parser: ParserHandle) -> ParserHandle {
        OptionalParser::new(parser).into_handle()
    }
}

/// Runs the inner parser and turns a failure into a match of nothing.
///
/// On success it passes the value through. On failure it consumes nothing
/// and yields [`ParserNoValue`], so check the output type to tell the two
/// apart.
#[derive(Clone)]
pub struct OptionalParser(ParserHandle);

impl OptionalParser {
    /// Makes `parser` optional.
    pub fn new(parser: ParserHandle) -> Self {
        Self(parser)
    }
}

impl Parser for OptionalParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        match self.0.parse(registry, input) {
            Ok(result) => Ok(result),
            Err(_) => Ok((input, ParserOutput::new(ParserNoValue).ok().unwrap())),
        }
    }

    fn extend(&self, parser: ParserHandle) {
        self.0.extend(parser);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserNoValue, ParserRegistry,
        optional::OptionalParser,
        shorthand::{lit, opt},
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_optional() {
        is_async::<OptionalParser>();

        let registry = ParserRegistry::default();
        let sentence = opt(lit("foo"));
        let (rest, value) = sentence.parse(&registry, "foobar").unwrap();
        assert_eq!(rest, "bar");
        assert!(value.consume::<String>().is_ok());
        let (rest, value) = sentence.parse(&registry, "barfoo").unwrap();
        assert_eq!(rest, "barfoo");
        assert!(value.consume::<ParserNoValue>().is_ok());
    }
}
