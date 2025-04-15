use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};

pub mod shorthand {
    use super::*;

    pub fn seq(values: impl IntoIterator<Item = ParserHandle>) -> ParserHandle {
        SequenceParser::from_iter(values).into_handle()
    }

    pub fn seq_del(
        delimiter: ParserHandle,
        values: impl IntoIterator<Item = ParserHandle>,
    ) -> ParserHandle {
        let mut result = SequenceDelimitedParser::new(delimiter);
        for parser in values {
            result.push(parser);
        }
        result.into_handle()
    }
}

#[derive(Default, Clone)]
pub struct SequenceParser {
    parsers: Vec<ParserHandle>,
}

impl SequenceParser {
    pub fn with(mut self, parser: ParserHandle) -> Self {
        self.push(parser);
        self
    }

    pub fn push(&mut self, parser: ParserHandle) {
        self.parsers.push(parser);
    }
}

impl Parser for SequenceParser {
    fn parse<'a>(&self, registry: &ParserRegistry, mut input: &'a str) -> ParseResult<'a> {
        let mut result = Vec::with_capacity(self.parsers.len());
        for parser in &self.parsers {
            let (new_input, value) = parser.parse(registry, input)?;
            input = new_input;
            result.push(value);
        }
        Ok((input, ParserOutput::new(result).ok().unwrap()))
    }
}

impl FromIterator<ParserHandle> for SequenceParser {
    fn from_iter<T: IntoIterator<Item = ParserHandle>>(iter: T) -> Self {
        Self {
            parsers: iter.into_iter().collect(),
        }
    }
}

#[derive(Clone)]
pub struct SequenceDelimitedParser {
    delimiter: ParserHandle,
    parsers: Vec<ParserHandle>,
}

impl SequenceDelimitedParser {
    pub fn new(delimiter: ParserHandle) -> Self {
        Self {
            delimiter,
            parsers: Default::default(),
        }
    }

    pub fn with(mut self, parser: ParserHandle) -> Self {
        self.push(parser);
        self
    }

    pub fn push(&mut self, parser: ParserHandle) {
        self.parsers.push(parser);
    }
}

impl Parser for SequenceDelimitedParser {
    fn parse<'a>(&self, registry: &ParserRegistry, mut input: &'a str) -> ParseResult<'a> {
        let mut result = Vec::with_capacity(self.parsers.len() * 2);
        for (index, parser) in self.parsers.iter().enumerate() {
            if index > 0 {
                let (new_input, _) = self.delimiter.parse(registry, input)?;
                input = new_input;
            }
            let (new_input, value) = parser.parse(registry, input)?;
            input = new_input;
            result.push(value);
        }
        Ok((input, ParserOutput::new(result).ok().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserNoValue, ParserOutput, ParserRegistry,
        sequence::{SequenceDelimitedParser, SequenceParser},
        shorthand::{lit, seq, seq_del, ws},
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_sequence() {
        is_async::<SequenceParser>();

        let registry = ParserRegistry::default();
        let sentence = seq([lit("foo"), ws(), lit("="), ws(), lit("bar")]);
        let (rest, result) = sentence.parse(&registry, "foo = bar").unwrap();
        assert_eq!(rest, "");
        let result = result.consume::<Vec<ParserOutput>>().ok().unwrap();
        assert_eq!(result.len(), 5);
        for result in result {
            assert!(result.read::<String>().is_some() || result.read::<ParserNoValue>().is_some());
        }
        assert_eq!(
            format!("{}", sentence.parse(&registry, "foo = ").err().unwrap()),
            "Expected 'bar'"
        );
    }

    #[test]
    fn test_sequence_delimited() {
        is_async::<SequenceDelimitedParser>();

        let registry = ParserRegistry::default();
        let sentence = seq_del(ws(), [lit("foo"), lit("="), lit("bar")]);
        let (rest, result) = sentence.parse(&registry, "foo = bar").unwrap();
        assert_eq!(rest, "");
        let result = result.consume::<Vec<ParserOutput>>().ok().unwrap();
        assert_eq!(result.len(), 3);
        for result in result {
            assert!(result.read::<String>().is_some() || result.read::<()>().is_some());
        }
        assert_eq!(
            format!("{}", sentence.parse(&registry, "foo = ").err().unwrap()),
            "Expected 'bar'"
        );
    }
}
