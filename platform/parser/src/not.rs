//! Negative lookahead.
use crate::{
    ParseResult, Parser, ParserExt, ParserHandle, ParserNoValue, ParserOutput, ParserRegistry,
};

/// Short constructors for this module.
pub mod shorthand {
    use super::*;

    /// See [`NotParser`].
    pub fn not(parser: ParserHandle) -> ParserHandle {
        NotParser::new(parser).into_handle()
    }
}

/// Succeeds exactly when the inner parser fails, and fails when it
/// succeeds.
///
/// Consumes nothing either way and yields [`ParserNoValue`], so it only
/// guards what may follow.
#[derive(Clone)]
pub struct NotParser(ParserHandle);

impl NotParser {
    /// Negates `parser`.
    pub fn new(parser: ParserHandle) -> Self {
        Self(parser)
    }
}

impl Parser for NotParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        match self.0.parse(registry, input) {
            Ok(_) => Err("Expected to not match input".into()),
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
        ParserRegistry,
        not::NotParser,
        shorthand::{lit, not, seq},
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_not() {
        is_async::<NotParser>();

        let registry = ParserRegistry::default();
        let sentence = seq([lit("foo"), not(lit("bar"))]);
        let (rest, _) = sentence.parse(&registry, "foozee").unwrap();
        assert_eq!(rest, "zee");
    }
}
