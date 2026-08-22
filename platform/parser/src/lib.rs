//! Parser combinators for building script frontends.
//!
//! A parser is anything implementing [`Parser`]: it takes the input that is
//! left and returns what remains plus one output value. Parsers are shared
//! as [`ParserHandle`], an `Arc`, so the same one can sit in many places of
//! a grammar.
//!
//! Outputs are type erased. A [`ParserOutput`] is a `DynamicManaged`, and
//! you take the value back out with `consume::<T>()` or `read::<T>()`. Most
//! built-in parsers produce a `String`, the repeating ones produce
//! `Vec<ParserOutput>`, and parsers with nothing to report produce
//! [`ParserNoValue`].
//!
//! ```
//! use intuicio_parser::{ParserRegistry, shorthand::*};
//!
//! let registry = ParserRegistry::default();
//! let sentence = seq([lit("foo"), ws(), lit("bar")]);
//! let (rest, _) = sentence.parse(&registry, "foo bar").unwrap();
//! assert_eq!(rest, "");
//! ```
//!
//! # Three ways to write a grammar
//!
//! Nested, as above: the [`shorthand`] module has a short constructor for
//! every parser and is meant to be glob imported.
//!
//! Named: put parsers in a [`ParserRegistry`] under an id and refer to them
//! with [`inject`](inject::shorthand::inject). This is also how recursion is
//! written, since a parser can inject itself.
//!
//! As text: [`generator`] reads a grammar written in its own small syntax
//! and builds the same parsers out of it.
//!
//! # What lives where
//!
//! | Job | Modules |
//! |---|---|
//! | match text | [`literal`], [`regex`] |
//! | order and choice | [`sequence`], [`alternation`], [`open_close`] |
//! | repetition | [`zero_or_more`], [`one_or_more`], [`repeat`], [`list`] |
//! | look without consuming | [`predict`], [`not`], [`optional`] |
//! | reshape the output | [`map`], [`inspect`], [`template`] |
//! | wire grammars together | [`inject`], [`slot`], [`extendable`], [`extension`] |
//! | expressions with precedence | [`pratt`] |
//! | grammar from text | [`generator`] |
//! | rules written in a script | [`dynamic`] |
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

/// Short constructor for every parser in the crate.
///
/// Meant to be glob imported, since grammars read better as nested calls
/// than as nested `::new` paths.
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

    /// See [`EndOfSourceParser`].
    pub fn eos() -> ParserHandle {
        EndOfSourceParser.into_handle()
    }

    /// See [`SourceParser`].
    pub fn source(parser: ParserHandle) -> ParserHandle {
        SourceParser::new(parser).into_handle()
    }

    /// See [`DebugParser`].
    pub fn debug(id: impl ToString, parser: ParserHandle) -> ParserHandle {
        DebugParser::new(id, parser).into_handle()
    }

    /// See [`EraseParser`].
    pub fn erase(parser: ParserHandle) -> ParserHandle {
        EraseParser::new(parser).into_handle()
    }

    /// Matches nothing, consumes nothing and yields [`ParserNoValue`].
    ///
    /// Useful as a stand-in wherever a parser is required but nothing should
    /// happen, such as the unused side of [`prefix`].
    pub fn ignore() -> ParserHandle {
        ().into_handle()
    }
}

use intuicio_data::managed::DynamicManaged;
use std::{
    any::{Any, TypeId},
    cell::Cell,
    collections::HashMap,
    error::Error,
    sync::{Arc, RwLock},
};

/// What a parser produces: one type-erased value.
///
/// Read it back with `read::<T>()` for a borrow or `consume::<T>()` to take
/// ownership.
pub type ParserOutput = DynamicManaged;
/// A shared parser, which is how grammars refer to their parts.
pub type ParserHandle = Arc<dyn Parser>;
/// What is left of the input, plus the value that was parsed.
pub type ParseResult<'a> = Result<(&'a str, ParserOutput), Box<dyn Error>>;

/// Placeholder output for parsers that match without producing anything,
/// such as whitespace.
pub struct ParserNoValue;

/// One step of a grammar.
///
/// Implement it for anything you want to plug in yourself. The crate's own
/// combinators are only implementations of this trait.
pub trait Parser: Send + Sync {
    /// Consumes a prefix of `input` and returns the rest with the value.
    ///
    /// Returning [`Err`] means no match, and the caller is free to try
    /// something else from the same position.
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a>;

    /// Adds a parser to this one after it was built, for grammars that grow
    /// later.
    ///
    /// Does nothing by default. Wrapping parsers pass it down to their inner
    /// parser. See [`extendable`] for the ones that act on it.
    #[allow(unused_variables)]
    fn extend(&self, parser: ParserHandle) {}
}

/// Turns a parser into a [`ParserHandle`], implemented for every [`Parser`].
pub trait ParserExt: Sized {
    /// Wraps `self` in an `Arc`.
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

/// Matches only when nothing is left, without consuming anything.
///
/// Put it last in a sequence to demand that the whole input was used.
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

/// Runs the inner parser and throws its value away, yielding
/// [`ParserNoValue`].
pub struct EraseParser {
    parser: ParserHandle,
}

impl EraseParser {
    /// Wraps `parser`.
    pub fn new(parser: ParserHandle) -> Self {
        Self { parser }
    }
}

impl Parser for EraseParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let (new_input, _) = self.parser.parse(registry, input)?;
        Ok((new_input, ParserOutput::new(ParserNoValue).ok().unwrap()))
    }
}

/// Runs the inner parser and yields the text it consumed, as a `String`,
/// instead of the value it produced.
pub struct SourceParser {
    parser: ParserHandle,
}

impl SourceParser {
    /// Wraps `parser`.
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

/// Prints the input before and the outcome after running the inner parser,
/// indented by nesting depth.
///
/// A development aid only. Nesting depth is counted per thread, so parsers
/// running side by side each keep their own indentation, even though their
/// lines still land on one output stream.
pub struct DebugParser {
    id: String,
    parser: ParserHandle,
}

impl DebugParser {
    /// Labels `parser` with `id` in the printout.
    pub fn new(id: impl ToString, parser: ParserHandle) -> Self {
        Self {
            id: id.to_string(),
            parser,
        }
    }
}

thread_local! {
    /// Nesting depth of the debug parsers running on this thread, used only to
    /// indent their output.
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

impl Parser for DebugParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let depth = DEPTH.with(|depth| {
            depth.set(depth.get() + 1);
            depth.get()
        });
        let ident = " ".repeat(depth);
        println!("{}< DEBUG `{}` | Before: {:?}", ident, self.id, input);
        let result = self.parser.parse(registry, input);
        match &result {
            Ok((rest, _)) => {
                println!("{}> DEBUG `{}` | OK After: {:?}", ident, self.id, rest);
            }
            Err(error) => {
                println!(
                    "{}> DEBUG `{}` | ERR After: {:?} | ERROR: {:?}",
                    ident, self.id, input, error
                );
            }
        }
        DEPTH.with(|depth| depth.set(depth.get() - 1));
        result
    }
}

/// Parsers and extension data, looked up by name at parse time.
///
/// This is what makes named and recursive grammars possible: a parser can
/// refer to another by id through [`inject`](crate::inject::shorthand::inject)
/// long before that id has anything behind it.
///
/// Everything is behind an `RwLock` and taken with `try_*`, so a call made
/// while the registry is locked elsewhere fails quietly rather than
/// blocking. In practice, finish building the registry before parsing with
/// it.
#[derive(Default)]
pub struct ParserRegistry {
    parsers: RwLock<HashMap<String, ParserHandle>>,
    extensions: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl ParserRegistry {
    /// [`ParserRegistry::add_parser`], builder style.
    pub fn with_parser(self, id: impl ToString, parser: ParserHandle) -> Self {
        self.add_parser(id, parser);
        self
    }

    /// [`ParserRegistry::add_extension`], builder style.
    pub fn with_extension<T: Send + Sync + 'static>(self, data: T) -> Self {
        self.add_extension::<T>(data);
        self
    }

    /// Stores `parser` under `id`, replacing whatever was there.
    ///
    /// Does nothing when the registry is locked elsewhere.
    pub fn add_parser(&self, id: impl ToString, parser: ParserHandle) {
        if let Ok(mut parsers) = self.parsers.try_write() {
            parsers.insert(id.to_string(), parser);
        }
    }

    /// Takes the parser stored under `id` out.
    ///
    /// Returns [`None`] when there is none, or when the registry is locked
    /// elsewhere.
    pub fn remove_parser(&self, id: impl AsRef<str>) -> Option<ParserHandle> {
        if let Ok(mut parsers) = self.parsers.try_write() {
            parsers.remove(id.as_ref())
        } else {
            None
        }
    }

    /// Returns a handle to the parser stored under `id`, if any.
    pub fn get_parser(&self, id: impl AsRef<str>) -> Option<ParserHandle> {
        self.parsers.try_read().ok()?.get(id.as_ref()).cloned()
    }

    /// Runs the parser stored under `id` over `input`.
    ///
    /// Fails when no parser is registered under that id.
    pub fn parse<'a>(&self, id: impl AsRef<str>, input: &'a str) -> ParseResult<'a> {
        if let Some(parser) = self.get_parser(id.as_ref()) {
            parser.parse(self, input)
        } else {
            Err(format!("Parser `{}` not found in registry", id.as_ref()).into())
        }
    }

    /// Passes `parser` to [`Parser::extend`] of the one stored under `id`.
    ///
    /// Fails when no parser is registered under that id. What extending does
    /// depends on the receiving parser. See [`extendable`].
    pub fn extend(&self, id: impl AsRef<str>, parser: ParserHandle) -> Result<(), Box<dyn Error>> {
        if let Some(extendable) = self.get_parser(id.as_ref()) {
            extendable.extend(parser);
            Ok(())
        } else {
            Err(format!("Parser '{}' not found in registry", id.as_ref()).into())
        }
    }

    /// Stores `data` as the extension for its own type, replacing any previous
    /// one.
    ///
    /// Returns `false` when the registry is locked elsewhere and nothing was
    /// stored.
    pub fn add_extension<T: Send + Sync + 'static>(&self, data: T) -> bool {
        if let Ok(mut extensions) = self.extensions.try_write() {
            extensions.insert(TypeId::of::<T>(), Arc::new(data));
            true
        } else {
            false
        }
    }

    /// Drops the extension stored for `T`.
    ///
    /// Returns `false` when the registry is locked elsewhere.
    pub fn remove_extension<T: 'static>(&self) -> bool {
        if let Ok(mut extensions) = self.extensions.try_write() {
            extensions.remove(&TypeId::of::<T>());
            true
        } else {
            false
        }
    }

    /// Returns the extension stored for `T`, if there is one.
    ///
    /// Parsers built with [`ext`](crate::extension::shorthand::ext) read their
    /// state this way.
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
        EndOfSourceParser, ParserNoValue, ParserRegistry, SourceParser,
        shorthand::{eos, erase, ignore, lit, number_int, seq, source},
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
    fn test_erase() {
        is_async::<()>();

        let registry = ParserRegistry::default();
        let sentence = erase(number_int());
        let (rest, result) = sentence.parse(&registry, "42 foo").unwrap();
        assert_eq!(rest, " foo");
        assert!(result.is::<ParserNoValue>());
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
