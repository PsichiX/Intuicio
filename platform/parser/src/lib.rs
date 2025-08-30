pub mod alternation;
pub mod dynamic;
pub mod extendable;
pub mod extension;
pub mod generator;
pub mod inject;
pub mod inspect;
pub mod list;
pub mod literal;
pub mod map;
pub mod not;
pub mod one_or_more;
pub mod open_close;
pub mod optional;
pub mod pratt;
pub mod predict;
pub mod regex;
pub mod repeat;
pub mod sequence;
pub mod slot;
pub mod template;
pub mod zero_or_more;

pub mod shorthand {
    use super::*;

    pub use crate::{
        alternation::shorthand::*, dynamic::shorthand::*, extendable::shorthand::*,
        extension::shorthand::*, inject::shorthand::*, inspect::shorthand::*, list::shorthand::*,
        literal::shorthand::*, map::shorthand::*, not::shorthand::*, one_or_more::shorthand::*,
        open_close::shorthand::*, optional::shorthand::*, pratt::shorthand::*,
        predict::shorthand::*, regex::shorthand::*, repeat::shorthand::*, sequence::shorthand::*,
        slot::shorthand::*, template::shorthand::*, zero_or_more::shorthand::*,
    };

    pub fn eos() -> ParserHandle {
        EndOfSourceParser.into_handle()
    }

    pub fn source(parser: ParserHandle) -> ParserHandle {
        SourceParser::new(parser).into_handle()
    }

    pub fn debug(id: impl ToString, parser: ParserHandle) -> ParserHandle {
        DebugParser::new(id, parser).into_handle()
    }

    pub fn ignore() -> ParserHandle {
        ().into_handle()
    }
}

use intuicio_data::managed::DynamicManaged;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    error::Error,
    sync::{Arc, RwLock},
};

pub type ParserOutput = DynamicManaged;
pub type ParserHandle = Arc<dyn Parser>;
pub type ParseResult<'a> = Result<(&'a str, ParserOutput), Box<dyn Error>>;

pub struct ParserNoValue;

pub trait Parser: Send + Sync {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a>;

    #[allow(unused_variables)]
    fn extend(&self, parser: ParserHandle) {}
}

pub trait ParserExt: Sized {
    fn into_handle(self) -> ParserHandle;
}

impl<T: Parser + 'static> ParserExt for T {
    fn into_handle(self) -> ParserHandle {
        Arc::new(self)
    }
}

impl Parser for () {
    fn parse<'a>(&self, _: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        Ok((input, ParserOutput::new(ParserNoValue).ok().unwrap()))
    }
}

pub struct EndOfSourceParser;

impl Parser for EndOfSourceParser {
    fn parse<'a>(&self, _: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        if input.is_empty() {
            Ok((input, ParserOutput::new(ParserNoValue).ok().unwrap()))
        } else {
            Err("Expected end of source".into())
        }
    }
}

pub struct SourceParser {
    parser: ParserHandle,
}

impl SourceParser {
    pub fn new(parser: ParserHandle) -> Self {
        Self { parser }
    }
}

impl Parser for SourceParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let before = input.len();
        let (new_input, _) = self.parser.parse(registry, input)?;
        let after = new_input.len();
        let size = before - after;
        Ok((
            new_input,
            ParserOutput::new(input[0..size].to_string()).ok().unwrap(),
        ))
    }
}

pub struct DebugParser {
    id: String,
    parser: ParserHandle,
}

impl DebugParser {
    pub fn new(id: impl ToString, parser: ParserHandle) -> Self {
        Self {
            id: id.to_string(),
            parser,
        }
    }
}

impl Parser for DebugParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        static mut IDENT: usize = 0;
        unsafe {
            IDENT += 1;
        }
        let ident = " ".repeat(unsafe { IDENT });
        println!("{}< DEBUG `{}` | Before: {:?}", ident, self.id, input);
        match self.parser.parse(registry, input) {
            Ok((input, result)) => {
                println!("{}> DEBUG `{}` | OK After: {:?}", ident, self.id, input);
                unsafe {
                    IDENT -= 1;
                }
                Ok((input, result))
            }
            Err(error) => {
                println!(
                    "{}> DEBUG `{}` | ERR After: {:?} | ERROR: {:?}",
                    ident, self.id, input, error
                );
                unsafe {
                    IDENT -= 1;
                }
                Err(error)
            }
        }
    }
}

#[derive(Default)]
pub struct ParserRegistry {
    parsers: RwLock<HashMap<String, ParserHandle>>,
    extensions: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl ParserRegistry {
    pub fn with_parser(self, id: impl ToString, parser: ParserHandle) -> Self {
        self.add_parser(id, parser);
        self
    }

    pub fn with_extension<T: Send + Sync + 'static>(self, data: T) -> Self {
        self.add_extension::<T>(data);
        self
    }

    pub fn add_parser(&self, id: impl ToString, parser: ParserHandle) {
        if let Ok(mut parsers) = self.parsers.try_write() {
            parsers.insert(id.to_string(), parser);
        }
    }

    pub fn remove_parser(&self, id: impl AsRef<str>) -> Option<ParserHandle> {
        if let Ok(mut parsers) = self.parsers.try_write() {
            parsers.remove(id.as_ref())
        } else {
            None
        }
    }

    pub fn get_parser(&self, id: impl AsRef<str>) -> Option<ParserHandle> {
        self.parsers.try_read().ok()?.get(id.as_ref()).cloned()
    }

    pub fn parse<'a>(&self, id: impl AsRef<str>, input: &'a str) -> ParseResult<'a> {
        if let Some(parser) = self.get_parser(id.as_ref()) {
            parser.parse(self, input)
        } else {
            Err(format!("Parser `{}` not found in registry", id.as_ref()).into())
        }
    }

    pub fn extend(&self, id: impl AsRef<str>, parser: ParserHandle) -> Result<(), Box<dyn Error>> {
        if let Some(extendable) = self.get_parser(id.as_ref()) {
            extendable.extend(parser);
            Ok(())
        } else {
            Err(format!("Parser '{}' not found in registry", id.as_ref()).into())
        }
    }

    pub fn add_extension<T: Send + Sync + 'static>(&self, data: T) -> bool {
        if let Ok(mut extensions) = self.extensions.try_write() {
            extensions.insert(TypeId::of::<T>(), Arc::new(data));
            true
        } else {
            false
        }
    }

    pub fn remove_extension<T: 'static>(&self) -> bool {
        if let Ok(mut extensions) = self.extensions.try_write() {
            extensions.remove(&TypeId::of::<T>());
            true
        } else {
            false
        }
    }

    pub fn extension<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.extensions
            .try_read()
            .ok()?
            .get(&TypeId::of::<T>())?
            .clone()
            .downcast::<T>()
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        EndOfSourceParser, ParserRegistry, SourceParser,
        shorthand::{eos, ignore, lit, number_int, seq, source},
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_end_of_source() {
        is_async::<EndOfSourceParser>();

        let registry = ParserRegistry::default();
        let sentence = seq([lit("foo"), eos()]);
        let (rest, _) = sentence.parse(&registry, "foo").unwrap();
        assert_eq!(rest, "");
        let sentence = eos();
        assert!(sentence.parse(&registry, "foo").is_err());
    }

    #[test]
    fn test_source() {
        is_async::<SourceParser>();

        let registry = ParserRegistry::default();
        let sentence = source(number_int());
        let (rest, result) = sentence.parse(&registry, "42 bar").unwrap();
        assert_eq!(rest, " bar");
        assert_eq!(result.read::<String>().unwrap().as_str(), "42");
    }

    #[test]
    fn test_ignore() {
        is_async::<()>();

        let registry = ParserRegistry::default();
        let sentence = ignore();
        let (rest, _) = sentence.parse(&registry, "foo").unwrap();
        assert_eq!(rest, "foo");
    }
}
