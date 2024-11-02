use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserRegistry};

pub mod shorthand {
    use super::*;

    pub fn pred(parser: ParserHandle) -> ParserHandle {
        PredictParser::new(parser).into_handle()
    }
}

#[derive(Clone)]
pub struct PredictParser(ParserHandle);

impl PredictParser {
    pub fn new(parser: ParserHandle) -> Self {
        Self(parser)
    }
}

impl Parser for PredictParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let (_, result) = self.0.parse(registry, input)?;
        Ok((input, result))
    }

    fn extend(&self, parser: ParserHandle) {
        self.0.extend(parser);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        predict::PredictParser,
        shorthand::{lit, pred, seq},
        ParserRegistry,
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_predict() {
        is_async::<PredictParser>();

        let registry = ParserRegistry::default();
        let sentence = seq([lit("foo"), pred(lit("bar"))]);
        let (rest, _) = sentence.parse(&registry, "foobar").unwrap();
        assert_eq!(rest, "bar");
    }
}
