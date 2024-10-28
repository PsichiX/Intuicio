use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};
use std::{
    error::Error,
    sync::{Arc, RwLock},
};

pub mod shorthand {
    use super::*;

    pub fn map<I: 'static, O: 'static>(
        parser: ParserHandle,
        f: impl FnMut(I) -> O + Send + Sync + 'static,
    ) -> ParserHandle {
        MapParser::new(parser, f).into_handle()
    }

    pub fn omap(
        parser: ParserHandle,
        f: impl FnMut(ParserOutput) -> ParserOutput + Send + Sync + 'static,
    ) -> ParserHandle {
        OutputMapParser::new(parser, f).into_handle()
    }

    pub fn map_err(
        parser: ParserHandle,
        f: impl FnMut(Box<dyn Error>) -> Box<dyn Error> + Send + Sync + 'static,
    ) -> ParserHandle {
        MapErrorParser::new(parser, f).into_handle()
    }
}

pub struct MapParser<I, O> {
    parser: ParserHandle,
    closure: Arc<RwLock<dyn FnMut(I) -> O + Send + Sync>>,
}

impl<I, O> MapParser<I, O> {
    pub fn new(parser: ParserHandle, f: impl FnMut(I) -> O + Send + Sync + 'static) -> Self {
        Self {
            parser,
            closure: Arc::new(RwLock::new(f)),
        }
    }
}

impl<I: 'static, O: 'static> Parser for MapParser<I, O> {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let (input, result) = self.parser.parse(registry, input)?;
        let result = match self.closure.write() {
            Ok(mut closure) => match result.consume::<I>() {
                Ok(result) => (closure)(result),
                Err(_) => {
                    return Err(format!(
                        "MapParser cannot downcast input from `{}` type",
                        std::any::type_name::<I>()
                    )
                    .into())
                }
            },
            Err(_) => return Err("MapParser cannot access closure mutably".into()),
        };
        Ok((input, ParserOutput::new(result).ok().unwrap()))
    }
}

pub struct OutputMapParser {
    parser: ParserHandle,
    closure: Arc<RwLock<dyn FnMut(ParserOutput) -> ParserOutput + Send + Sync>>,
}

impl OutputMapParser {
    pub fn new(
        parser: ParserHandle,
        f: impl FnMut(ParserOutput) -> ParserOutput + Send + Sync + 'static,
    ) -> Self {
        Self {
            parser,
            closure: Arc::new(RwLock::new(f)),
        }
    }
}

impl Parser for OutputMapParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let (input, result) = self.parser.parse(registry, input)?;
        let result = match self.closure.write() {
            Ok(mut closure) => (closure)(result),
            Err(_) => return Err("OutputMapParser cannot access closure mutably".into()),
        };
        Ok((input, result))
    }
}

pub struct MapErrorParser {
    parser: ParserHandle,
    #[allow(clippy::type_complexity)]
    closure: Arc<RwLock<dyn FnMut(Box<dyn Error>) -> Box<dyn Error> + Send + Sync>>,
}

impl MapErrorParser {
    pub fn new(
        parser: ParserHandle,
        f: impl FnMut(Box<dyn Error>) -> Box<dyn Error> + Send + Sync + 'static,
    ) -> Self {
        Self {
            parser,
            closure: Arc::new(RwLock::new(f)),
        }
    }
}

impl Parser for MapErrorParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        match self.parser.parse(registry, input) {
            Ok(result) => Ok(result),
            Err(error) => match self.closure.write() {
                Ok(mut closure) => Err((closure)(error)),
                Err(_) => Err("MapErrorParser cannot access closure mutably".into()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        map::{MapErrorParser, MapParser, OutputMapParser},
        shorthand::{map, map_err, number_float, omap},
        ParserOutput, ParserRegistry,
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_map() {
        is_async::<MapParser<(), ()>>();

        let registry = ParserRegistry::default();
        let number = map(number_float(), |value: String| {
            value.parse::<f32>().unwrap()
        });
        assert_eq!(
            number
                .parse(&registry, "-4.2e1")
                .unwrap()
                .1
                .consume::<f32>()
                .ok()
                .unwrap(),
            -42.0
        );
    }

    #[test]
    fn test_omap() {
        is_async::<OutputMapParser>();

        let registry = ParserRegistry::default();
        let number = omap(number_float(), |value| {
            ParserOutput::new(
                value
                    .consume::<String>()
                    .ok()
                    .unwrap()
                    .parse::<f32>()
                    .unwrap(),
            )
            .ok()
            .unwrap()
        });
        assert_eq!(
            number
                .parse(&registry, "-4.2e1")
                .unwrap()
                .1
                .consume::<f32>()
                .ok()
                .unwrap(),
            -42.0
        );
    }

    #[test]
    fn test_map_error() {
        is_async::<MapErrorParser>();

        let registry = ParserRegistry::default();
        let number = map_err(number_float(), |_| "Expected float number".into());
        assert_eq!(
            format!("{}", number.parse(&registry, "foo").err().unwrap()),
            "Expected float number"
        );
    }
}
