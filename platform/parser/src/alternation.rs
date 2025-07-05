use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserRegistry};
use std::fmt::Write;

pub mod shorthand {
    use super::*;

    pub fn alt(values: impl IntoIterator<Item = ParserHandle>) -> ParserHandle {
        AlternationParser::from_iter(values).into_handle()
    }
}

#[derive(Default, Clone)]
pub struct AlternationParser {
    parsers: Vec<ParserHandle>,
}

impl AlternationParser {
    pub fn with(mut self, parser: ParserHandle) -> Self {
        self.append(parser);
        self
    }

    pub fn append(&mut self, parser: ParserHandle) {
        self.parsers.push(parser);
    }

    pub fn prepend(&mut self, parser: ParserHandle) {
        self.parsers.insert(0, parser);
    }
}

impl Parser for AlternationParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let mut errors = String::new();
        for parser in &self.parsers {
            match parser.parse(registry, input) {
                Ok(result) => {
                    return Ok(result);
                }
                Err(error) => {
                    if errors.is_empty() {
                        write!(&mut errors, "{error}")?;
                    } else {
                        write!(&mut errors, "\nOr: {error}")?;
                    }
                }
            }
        }
        Err(errors.into())
    }
}

impl FromIterator<ParserHandle> for AlternationParser {
    fn from_iter<T: IntoIterator<Item = ParserHandle>>(iter: T) -> Self {
        Self {
            parsers: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserRegistry,
        alternation::AlternationParser,
        shorthand::{alt, lit},
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_alternation() {
        is_async::<AlternationParser>();

        let registry = ParserRegistry::default();
        let sentence = alt([lit("foo"), lit("bar")]);
        let (rest, result) = sentence.parse(&registry, "foo").unwrap();
        assert_eq!(rest, "");
        result.consume::<String>().ok().unwrap();
        let (rest, result) = sentence.parse(&registry, "bar").unwrap();
        assert_eq!(rest, "");
        result.consume::<String>().ok().unwrap();
        assert_eq!(
            format!("{}", sentence.parse(&registry, "zee").err().unwrap()),
            "Expected 'foo'\nOr: Expected 'bar'"
        );
    }
}
