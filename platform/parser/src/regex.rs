//! Matching text with regular expressions.
//!
//! Every pattern is anchored to the front of the input and compiled once,
//! then kept in a per-thread cache keyed by the pattern text.
//!
//! Most of the named parsers in [`shorthand`] are ready-made patterns, and
//! they are what a grammar reaches for instead of writing regexes by hand.
use crate::{
    ParseResult, Parser, ParserExt, ParserHandle, ParserNoValue, ParserOutput, ParserRegistry,
};
use std::{cell::RefCell, collections::HashMap, sync::Arc};

/// Short constructors for this module.
pub mod shorthand {
    use super::*;
    use crate::shorthand::map;

    /// See [`RegexParser`].
    pub fn regex(pattern: impl AsRef<str>) -> ParserHandle {
        RegexParser::new(pattern).into_handle()
    }

    /// See [`RegexParser::new_capture`].
    pub fn regex_capture(pattern: impl AsRef<str>, capture: impl ToString) -> ParserHandle {
        RegexParser::new_capture(pattern, capture).into_handle()
    }

    /// Any single character.
    pub fn any() -> ParserHandle {
        regex(r".")
    }

    /// One line break character.
    pub fn nl() -> ParserHandle {
        regex(r"[\r\n]")
    }

    /// One hexadecimal digit.
    pub fn digit_hex() -> ParserHandle {
        regex(r"[0-9a-fA-F]")
    }

    /// One decimal digit.
    pub fn digit() -> ParserHandle {
        regex(r"\d")
    }

    /// Digits with no sign.
    pub fn number_int_pos() -> ParserHandle {
        regex(r"\d+")
    }

    /// Digits with an optional leading `-`.
    pub fn number_int() -> ParserHandle {
        regex(r"-?\d+")
    }

    /// An integer with an optional fraction and exponent, as in `-4.2e1`.
    pub fn number_float() -> ParserHandle {
        regex(r"-?\d+(\.\d+(e-?\d+)?)?")
    }

    /// One letter, digit or underscore.
    pub fn alphanum() -> ParserHandle {
        regex(r"\w")
    }

    /// One lowercase ASCII letter.
    pub fn alpha_low() -> ParserHandle {
        regex(r"[a-z]")
    }

    /// One uppercase ASCII letter.
    pub fn alpha_up() -> ParserHandle {
        regex(r"[A-Z]")
    }

    /// One ASCII letter of either case.
    pub fn alpha() -> ParserHandle {
        regex(r"[a-zA-Z]")
    }

    /// A run of letters, digits and underscores.
    pub fn word() -> ParserHandle {
        regex(r"\w+")
    }

    /// Text between two markers, with escape sequences resolved.
    ///
    /// The markers are matched literally and left out of the result, and the
    /// content may not contain any character of the closing marker.
    pub fn string(open: &str, close: &str) -> ParserHandle {
        let open = open.escape_unicode().to_string();
        let close = close.escape_unicode().to_string();
        let pattern = format!("{open}(?<content>[^{close}]*){close}");
        map(regex_capture(pattern, "content"), move |value: String| {
            snailquote::unescape(&value).unwrap()
        })
    }

    /// One character an identifier may start with.
    pub fn id_start() -> ParserHandle {
        regex(r"[a-zA-Z_]")
    }

    /// The rest of an identifier, possibly empty.
    pub fn id_continue() -> ParserHandle {
        regex(r"[0-9a-zA-Z_]*")
    }

    /// A whole identifier: a letter or underscore, then the rest.
    pub fn id() -> ParserHandle {
        regex(r"[a-zA-Z_][0-9a-zA-Z_]*")
    }

    /// See [`WhiteSpaceParser`].
    pub fn ws() -> ParserHandle {
        WhiteSpaceParser::default().into_handle()
    }

    /// See [`OptionalWhiteSpaceParser`].
    pub fn ows() -> ParserHandle {
        OptionalWhiteSpaceParser::default().into_handle()
    }
}

thread_local! {
    /// Compiled patterns, kept per thread and keyed by the pattern text, so a
    /// grammar rebuilt many times does not recompile the same regex.
    static REGEX_CACHE: RefCell<HashMap<String, Arc<regex::Regex>>> = Default::default();
}

/// Matches a regular expression at the front of the input.
///
/// Yields the matched text, or the named capture group when one was asked
/// for. Patterns are anchored automatically, so `\w+` means "a word right
/// here", not "a word somewhere ahead".
#[derive(Clone)]
pub struct RegexParser {
    regex: Arc<regex::Regex>,
    capture: Option<String>,
}

impl RegexParser {
    /// Matches `pattern` and yields the whole match.
    ///
    /// # Panics
    ///
    /// Panics when `pattern` is not a valid regular expression.
    pub fn new(pattern: impl AsRef<str>) -> Self {
        let pattern = pattern.as_ref();
        REGEX_CACHE.with_borrow_mut(|cache| {
            if let Some(cached) = cache.get(pattern) {
                return Self {
                    regex: cached.clone(),
                    capture: None,
                };
            }
            let regex = Arc::new(
                regex::Regex::new(&format!(r"^{}", pattern)).expect("Expected valid regex"),
            );
            cache.insert(pattern.to_string(), regex.clone());
            Self {
                regex,
                capture: None,
            }
        })
    }

    /// Matches `pattern` but yields the named capture group `capture`.
    ///
    /// The whole match is still what gets consumed. A group that did not take
    /// part yields an empty string.
    ///
    /// # Panics
    ///
    /// Panics when `pattern` is not a valid regular expression.
    pub fn new_capture(pattern: impl AsRef<str>, capture: impl ToString) -> Self {
        let pattern = pattern.as_ref();
        let capture = capture.to_string();
        REGEX_CACHE.with_borrow_mut(|cache| {
            if let Some(cached) = cache.get(pattern) {
                return Self {
                    regex: cached.clone(),
                    capture: Some(capture),
                };
            }
            let regex = Arc::new(
                regex::Regex::new(&format!(r"^{}", pattern)).expect("Expected valid regex"),
            );
            cache.insert(pattern.to_string(), regex.clone());
            Self {
                regex,
                capture: Some(capture),
            }
        })
    }
}

impl Parser for RegexParser {
    fn parse<'a>(&self, _: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        if let Some(capture) = self.capture.as_deref() {
            if let Some(cap) = self.regex.captures(input) {
                Ok((
                    &input[cap.get(0).unwrap().end()..],
                    ParserOutput::new(
                        cap.name(capture)
                            .map(|mat| mat.as_str())
                            .unwrap_or("")
                            .to_owned(),
                    )
                    .ok()
                    .unwrap(),
                ))
            } else {
                Err(format!(
                    "Expected regex match '{}' with capture: '{}'",
                    self.regex, capture
                )
                .into())
            }
        } else if let Some(mat) = self.regex.find(input) {
            Ok((
                &input[mat.end()..],
                ParserOutput::new(mat.as_str().to_owned()).ok().unwrap(),
            ))
        } else {
            Err(format!("Expected regex match '{}'", self.regex).into())
        }
    }
}

/// Matches one or more whitespace characters, yielding [`ParserNoValue`].
#[derive(Clone)]
pub struct WhiteSpaceParser(RegexParser);

impl Default for WhiteSpaceParser {
    fn default() -> Self {
        Self(RegexParser::new(r"\s+"))
    }
}

impl Parser for WhiteSpaceParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        match self.0.parse(registry, input) {
            Ok((rest, _)) => Ok((rest, ParserOutput::new(ParserNoValue).ok().unwrap())),
            Err(error) => Err(error),
        }
    }
}

/// Matches any amount of whitespace including none, yielding
/// [`ParserNoValue`].
#[derive(Clone)]
pub struct OptionalWhiteSpaceParser(RegexParser);

impl Default for OptionalWhiteSpaceParser {
    fn default() -> Self {
        Self(RegexParser::new(r"\s*"))
    }
}

impl Parser for OptionalWhiteSpaceParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        match self.0.parse(registry, input) {
            Ok((rest, _)) => Ok((rest, ParserOutput::new(ParserNoValue).ok().unwrap())),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserRegistry,
        regex::{OptionalWhiteSpaceParser, RegexParser, WhiteSpaceParser},
        shorthand::{digit_hex, ows, regex, regex_capture, string, ws},
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_regex() {
        is_async::<RegexParser>();
        is_async::<WhiteSpaceParser>();
        is_async::<OptionalWhiteSpaceParser>();

        let registry = ParserRegistry::default();

        let keyword = regex_capture(r"\s+(?<name>\w+)\s+", "name");
        let (rest, result) = keyword.parse(&registry, " foo ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.read::<String>().unwrap().as_str(), "foo");

        let keyword = string("`", "`");
        let (rest, result) = keyword.parse(&registry, "`Hello World!`").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.read::<String>().unwrap().as_str(), "Hello World!");

        let keyword = string("(", ")");
        let (rest, result) = keyword.parse(&registry, "(Hello World!)").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.read::<String>().unwrap().as_str(), "Hello World!");

        let keyword = regex(r"\w+");
        assert_eq!(keyword.parse(&registry, "foo bar").unwrap().0, " bar");

        let ws = ws();
        assert_eq!(ws.parse(&registry, "   \t  \n").unwrap().0, "");
        assert_eq!(
            format!("{}", ws.parse(&registry, "a").err().unwrap()),
            "Expected regex match '^\\s+'"
        );

        let ows = ows();
        assert_eq!(ows.parse(&registry, "   \t  \n").unwrap().0, "");
        assert_eq!(ows.parse(&registry, "foo").unwrap().0, "foo");
    }

    #[test]
    fn test_digit_hex() {
        let registry = ParserRegistry::default();

        let digit = digit_hex();
        let (rest, result) = digit.parse(&registry, "aF0").unwrap();
        assert_eq!(rest, "F0");
        assert_eq!(result.read::<String>().unwrap().as_str(), "a");
        assert!(digit.parse(&registry, "g").is_err());
    }
}
