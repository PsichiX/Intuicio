use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};

pub mod shorthand {
    use super::*;

    pub fn oom(parser: ParserHandle) -> ParserHandle {
        OneOrMoreParser::new(parser).into_handle()
    }
}

#[derive(Clone)]
pub struct OneOrMoreParser(ParserHandle);

impl OneOrMoreParser {
    pub fn new(parser: ParserHandle) -> Self {
        Self(parser)
    }
}

impl Parser for OneOrMoreParser {
    fn parse<'a>(&self, registry: &ParserRegistry, mut input: &'a str) -> ParseResult<'a> {
        let (new_input, value) = self.0.parse(registry, input)?;
        input = new_input;
        let mut result = vec![value];
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
        one_or_more::OneOrMoreParser,
        shorthand::{lit, oom},
        ParserOutput, ParserRegistry,
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_one_or_more() {
        is_async::<OneOrMoreParser>();

        let registry = ParserRegistry::default();
        let sentence = oom(lit("foo"));
        let (rest, result) = sentence.parse(&registry, "foofoofoo").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<Vec<ParserOutput>>().ok().unwrap().len(), 3);
        assert_eq!(
            format!("{}", sentence.parse(&registry, " asd ").err().unwrap()),
            "Expected 'foo'"
        );
    }
}
