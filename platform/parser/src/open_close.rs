use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserRegistry};

pub mod shorthand {
    use super::*;
    use crate::shorthand::ignore;

    pub fn oc(parser: ParserHandle, open: ParserHandle, close: ParserHandle) -> ParserHandle {
        OpenCloseParser::new(parser, open, close).into_handle()
    }

    pub fn prefix(parser: ParserHandle, prefix: ParserHandle) -> ParserHandle {
        oc(parser, prefix, ignore())
    }

    pub fn suffix(parser: ParserHandle, suffix: ParserHandle) -> ParserHandle {
        oc(parser, ignore(), suffix)
    }
}

#[derive(Clone)]
pub struct OpenCloseParser {
    parser: ParserHandle,
    open: ParserHandle,
    close: ParserHandle,
}

impl OpenCloseParser {
    pub fn new(parser: ParserHandle, open: ParserHandle, close: ParserHandle) -> Self {
        Self {
            parser,
            open,
            close,
        }
    }
}

impl Parser for OpenCloseParser {
    fn parse<'a>(&self, registry: &ParserRegistry, mut input: &'a str) -> ParseResult<'a> {
        let (new_input, _) = self.open.parse(registry, input)?;
        input = new_input;
        let (new_input, result) = self.parser.parse(registry, input)?;
        input = new_input;
        let (new_input, _) = self.close.parse(registry, input)?;
        input = new_input;
        Ok((input, result))
    }

    fn extend(&self, parser: ParserHandle) {
        self.parser.extend(parser);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserRegistry,
        open_close::OpenCloseParser,
        shorthand::{lit, oc, prefix, suffix},
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_open_close() {
        is_async::<OpenCloseParser>();

        let registry = ParserRegistry::default();
        let sequence = oc(lit("foo"), lit("("), lit(")"));
        let (rest, result) = sequence.parse(&registry, "(foo)").unwrap();
        assert_eq!(rest, "");
        let result = result.consume::<String>().ok().unwrap();
        assert_eq!(result.as_str(), "foo");
    }

    #[test]
    fn test_prefix() {
        let registry = ParserRegistry::default();
        let sequence = prefix(lit("foo"), lit("("));
        let (rest, result) = sequence.parse(&registry, "(foo").unwrap();
        assert_eq!(rest, "");
        let result = result.consume::<String>().ok().unwrap();
        assert_eq!(result.as_str(), "foo");
    }

    #[test]
    fn test_suffix() {
        let registry = ParserRegistry::default();
        let sequence = suffix(lit("foo"), lit(")"));
        let (rest, result) = sequence.parse(&registry, "foo)").unwrap();
        assert_eq!(rest, "");
        let result = result.consume::<String>().ok().unwrap();
        assert_eq!(result.as_str(), "foo");
    }
}
