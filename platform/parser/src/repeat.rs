use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};

pub mod shorthand {
    use super::*;

    pub fn rep(parser: ParserHandle, occurrences: usize) -> ParserHandle {
        RepeatParser::new(parser, occurrences).into_handle()
    }
}

#[derive(Clone)]
pub struct RepeatParser {
    parser: ParserHandle,
    occurrences: usize,
}

impl RepeatParser {
    pub fn new(parser: ParserHandle, occurrences: usize) -> Self {
        Self {
            parser,
            occurrences,
        }
    }
}

impl Parser for RepeatParser {
    fn parse<'a>(&self, registry: &ParserRegistry, mut input: &'a str) -> ParseResult<'a> {
        let mut result = Vec::with_capacity(self.occurrences);
        for _ in 0..self.occurrences {
            let (new_input, value) = self.parser.parse(registry, input)?;
            result.push(value);
            input = new_input;
        }
        Ok((input, ParserOutput::new(result).ok().unwrap()))
    }

    fn extend(&self, parser: ParserHandle) {
        self.parser.extend(parser);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        repeat::RepeatParser,
        shorthand::{lit, rep},
        ParserOutput, ParserRegistry,
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_repeat() {
        is_async::<RepeatParser>();

        let registry = ParserRegistry::default();
        let sentence = rep(lit("foo"), 3);
        let (rest, result) = sentence.parse(&registry, "foofoofoo").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<Vec<ParserOutput>>().ok().unwrap().len(), 3);
        assert_eq!(
            format!("{}", sentence.parse(&registry, "foo").err().unwrap()),
            "Expected 'foo'"
        );
    }
}
