//! A hole in a grammar that is filled in later.
use crate::{
    ParseResult, Parser, ParserExt, ParserHandle, ParserNoValue, ParserOutput, ParserRegistry,
};
use std::sync::RwLock;

/// Short constructors for this module.
pub mod shorthand {
    use super::*;

    /// See [`SlotParser`].
    pub fn slot(parser: ParserHandle) -> ParserHandle {
        SlotParser::new(parser).into_handle()
    }

    /// An empty [`SlotParser`], to be filled in later.
    pub fn slot_empty() -> ParserHandle {
        SlotParser::default().into_handle()
    }
}

/// Holds a parser that can be swapped at any time, even mid-grammar.
///
/// Lets a grammar be built before all of its parts exist, and lets a part
/// be replaced afterwards. [`Parser::extend`] on a slot sets its contents.
///
/// An empty slot matches nothing and yields [`ParserNoValue`], unless it
/// was built with [`SlotParser::parse_error_when_empty`].
#[derive(Default)]
pub struct SlotParser {
    parser: RwLock<Option<ParserHandle>>,
    parse_error_when_empty: bool,
}

impl Clone for SlotParser {
    fn clone(&self) -> Self {
        Self {
            parser: RwLock::new(self.parser.read().unwrap().clone()),
            parse_error_when_empty: self.parse_error_when_empty,
        }
    }
}

impl SlotParser {
    /// Starts out holding `parser`.
    pub fn new(parser: ParserHandle) -> Self {
        Self {
            parser: RwLock::new(Some(parser)),
            parse_error_when_empty: false,
        }
    }

    /// Makes an empty slot fail instead of quietly matching nothing.
    pub fn parse_error_when_empty(mut self) -> Self {
        self.parse_error_when_empty = true;
        self
    }

    /// Returns `true` while a parser is in the slot.
    pub fn has(&self) -> bool {
        self.parser.read().map(|v| v.is_some()).unwrap_or_default()
    }

    /// Puts `parser` in the slot, replacing what was there.
    pub fn set(&self, parser: ParserHandle) {
        if let Ok(mut v) = self.parser.write() {
            *v = Some(parser);
        }
    }

    /// Returns the parser in the slot, if any.
    pub fn get(&self) -> Option<ParserHandle> {
        self.parser.read().ok()?.clone()
    }

    /// Replaces the contents with whatever `f` makes of them, so a slot can be
    /// wrapped in terms of itself.
    pub fn transform(&self, mut f: impl FnMut(Option<ParserHandle>) -> Option<ParserHandle>) {
        if let Ok(mut inner) = self.parser.write() {
            *inner = f(inner.clone());
        }
    }
}

impl Parser for SlotParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        if let Ok(inner) = self.parser.read() {
            if let Some(parser) = inner.as_ref() {
                parser.parse(registry, input)
            } else if self.parse_error_when_empty {
                Err("SlotParser has no parser".into())
            } else {
                Ok((input, ParserOutput::new(ParserNoValue).ok().unwrap()))
            }
        } else {
            Err("Slot parser cannot be accessed".into())
        }
    }

    fn extend(&self, parser: ParserHandle) {
        self.set(parser);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserNoValue, ParserRegistry,
        shorthand::{lit, slot_empty},
        slot::SlotParser,
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_slot() {
        is_async::<SlotParser>();

        let registry = ParserRegistry::default();
        let keyword_foo = lit("foo");
        let slot = slot_empty();
        let (rest, value) = slot.parse(&registry, "foobar").unwrap();
        assert_eq!(rest, "foobar");
        value.consume::<ParserNoValue>().ok().unwrap();
        slot.extend(keyword_foo);
        let (rest, value) = slot.parse(&registry, "foobar").unwrap();
        assert_eq!(rest, "bar");
        assert!(value.consume::<String>().is_ok());
        assert_eq!(
            format!("{}", slot.parse(&registry, "barfoo").err().unwrap()),
            "Expected 'foo'"
        );
    }
}
