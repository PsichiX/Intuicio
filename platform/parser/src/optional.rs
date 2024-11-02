use crate::{
    ParseResult, Parser, ParserExt, ParserHandle, ParserNoValue, ParserOutput, ParserRegistry,
};

pub mod shorthand {
    use super::*;

    pub fn opt(parser: ParserHandle) -> ParserHandle {
        OptionalParser::new(parser).into_handle()
    }
}

#[derive(Clone)]
pub struct OptionalParser(ParserHandle);

impl OptionalParser {
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
        optional::OptionalParser,
        shorthand::{lit, opt},
        ParserNoValue, ParserRegistry,
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
