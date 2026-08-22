//! Matching repeated items with something between them.
use crate::{
    ParseResult, Parser, ParserExt, ParserHandle, ParserNoValue, ParserOutput, ParserRegistry,
};

/// Short constructors for this module.
pub mod shorthand {
    use super::*;

    /// See [`ListParser`].
    pub fn list(item: ParserHandle, delimiter: ParserHandle, permissive: bool) -> ParserHandle {
        ListParser::new(item, delimiter, permissive).into_handle()
    }

    /// [`ListParser`] that drops empty outputs.
    pub fn list_inv(item: ParserHandle, delimiter: ParserHandle, permissive: bool) -> ParserHandle {
        ListParser::new(item, delimiter, permissive)
            .ignore_no_value(true)
            .into_handle()
    }
}

/// Matches items separated by a delimiter, yielding a `Vec<ParserOutput>`.
///
/// Never fails: an input where even the first item does not match gives an
/// empty list. `permissive` decides what a delimiter with no item after it
/// means - `true` ends the list and leaves the delimiter unconsumed, which
/// allows a trailing one, `false` makes the whole parse fail.
#[derive(Clone)]
pub struct ListParser {
    item: ParserHandle,
    delimiter: ParserHandle,
    permissive: bool,
    ignore_no_value: bool,
}

impl ListParser {
    /// Matches `item` repeatedly, separated by `delimiter`.
    pub fn new(item: ParserHandle, delimiter: ParserHandle, permissive: bool) -> Self {
        Self {
            item,
            delimiter,
            permissive,
            ignore_no_value: false,
        }
    }

    /// Sets whether [`ParserNoValue`] outputs are left out of the result.
    pub fn ignore_no_value(mut self, ignore: bool) -> Self {
        self.ignore_no_value = ignore;
        self
    }
}

impl Parser for ListParser {
    fn parse<'a>(&self, registry: &ParserRegistry, mut input: &'a str) -> ParseResult<'a> {
        let mut result = vec![];
        if let Ok((new_input, value)) = self.item.parse(registry, input) {
            input = new_input;
            if !self.ignore_no_value || !value.is::<ParserNoValue>() {
                result.push(value);
            }
            while let Ok((new_input, _)) = self.delimiter.parse(registry, input) {
                match self.item.parse(registry, new_input) {
                    Ok((new_input, value)) => {
                        input = new_input;
                        if !self.ignore_no_value || !value.is::<ParserNoValue>() {
                            result.push(value);
                        }
                    }
                    Err(error) => {
                        if self.permissive {
                            break;
                        } else {
                            return Err(error);
                        }
                    }
                }
            }
        }
        Ok((input, ParserOutput::new(result).ok().unwrap()))
    }

    fn extend(&self, parser: ParserHandle) {
        self.item.extend(parser);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserOutput, ParserRegistry,
        list::ListParser,
        shorthand::{alt, list, lit, ows},
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_list() {
        is_async::<ListParser>();

        let registry = ParserRegistry::default();
        let sentence = list(alt([lit("foo"), lit("bar")]), ows(), true);
        let (rest, _) = sentence.parse(&registry, "").unwrap();
        assert_eq!(rest, "");
        let (rest, result) = sentence.parse(&registry, "foobar foozee").unwrap();
        assert_eq!(rest, "zee");
        let result = result.consume::<Vec<ParserOutput>>().ok().unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].read::<String>().unwrap().as_str(), "foo");
        assert_eq!(result[1].read::<String>().unwrap().as_str(), "bar");
        assert_eq!(result[2].read::<String>().unwrap().as_str(), "foo");
    }
}
