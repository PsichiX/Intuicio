use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};

pub mod shorthand {
    use super::*;

    pub fn zom(parser: ParserHandle) -> ParserHandle {
        ZeroOrMoreParser::new(parser).into_handle()
    }
}

#[derive(Clone)]
pub struct ZeroOrMoreParser(ParserHandle);

impl ZeroOrMoreParser {
    pub fn new(parser: ParserHandle) -> Self {
        Self(parser)
    }
}

impl Parser for ZeroOrMoreParser {
    fn parse<'a>(&self, registry: &ParserRegistry, mut input: &'a str) -> ParseResult<'a> {
        let mut result = vec![];
        while let Ok((new_input, value)) = self.0.parse(registry, input) {
            result.push(value);
            input = new_input;
        }
        Ok((input, ParserOutput::new(result).ok().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        shorthand::{lit, zom},
        zero_or_more::ZeroOrMoreParser,
        ParserOutput, ParserRegistry,
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_zero_or_more() {
        is_async::<ZeroOrMoreParser>();

        let registry = ParserRegistry::default();
        let sentence = zom(lit("foo"));
        let (rest, _) = sentence.parse(&registry, "").unwrap();
        assert_eq!(rest, "");
        let (rest, result) = sentence.parse(&registry, "foofoofoo").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<Vec<ParserOutput>>().ok().unwrap().len(), 3);
        let (rest, result) = sentence.parse(&registry, " asd ").unwrap();
        assert_eq!(rest, " asd ");
        assert!(result
            .consume::<Vec<ParserOutput>>()
            .ok()
            .unwrap()
            .is_empty());
    }
}
