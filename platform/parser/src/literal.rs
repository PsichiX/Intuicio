use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};
use std::borrow::Cow;

pub mod shorthand {
    use super::*;

    pub fn lit(value: impl Into<Cow<'static, str>>) -> ParserHandle {
        LiteralParser::new(value).into_handle()
    }
}

#[derive(Clone)]
pub struct LiteralParser {
    literal: Cow<'static, str>,
}

impl LiteralParser {
    pub fn new(value: impl Into<Cow<'static, str>>) -> Self {
        Self {
            literal: value.into(),
        }
    }
}

impl Parser for LiteralParser {
    fn parse<'a>(&self, _: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        if input.starts_with(&*self.literal) {
            Ok((
                &input[self.literal.len()..],
                ParserOutput::new(self.literal.to_string()).ok().unwrap(),
            ))
        } else {
            Err(format!("Expected '{}'", self.literal).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{literal::LiteralParser, shorthand::lit, ParserRegistry};

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_literal() {
        is_async::<LiteralParser>();

        let registry = ParserRegistry::default();
        let keyword = lit("foo");
        let (rest, result) = keyword.parse(&registry, "foo bar").unwrap();
        assert_eq!(rest, " bar");
        let result = result.consume::<String>().ok().unwrap();
        assert_eq!(result.as_str(), "foo");
    }
}
