use crate::{
    ParseResult, Parser, ParserExt, ParserHandle, ParserNoValue, ParserOutput, ParserRegistry,
};

pub mod shorthand {
    use super::*;
    use crate::shorthand::map;

    pub fn regex(pattern: impl AsRef<str>) -> ParserHandle {
        RegexParser::new(pattern).into_handle()
    }

    pub fn regex_capture(pattern: impl AsRef<str>, capture: impl ToString) -> ParserHandle {
        RegexParser::new_capture(pattern, capture).into_handle()
    }

    pub fn any() -> ParserHandle {
        regex(r".")
    }

    pub fn nl() -> ParserHandle {
        regex(r"[\r\n]")
    }

    pub fn digit_hex() -> ParserHandle {
        regex(r"[0-9a-fA-F]&")
    }

    pub fn digit() -> ParserHandle {
        regex(r"\d")
    }

    pub fn number_int_pos() -> ParserHandle {
        regex(r"\d+")
    }

    pub fn number_int() -> ParserHandle {
        regex(r"-?\d+")
    }

    pub fn number_float() -> ParserHandle {
        regex(r"-?\d+(\.\d+(e-?\d+)?)?")
    }

    pub fn alphanum() -> ParserHandle {
        regex(r"\w")
    }

    pub fn alpha_low() -> ParserHandle {
        regex(r"[a-z]")
    }

    pub fn alpha_up() -> ParserHandle {
        regex(r"[A-Z]")
    }

    pub fn alpha() -> ParserHandle {
        regex(r"[a-zA-Z]")
    }

    pub fn word() -> ParserHandle {
        regex(r"\w+")
    }

    pub fn string(open: &str, close: &str) -> ParserHandle {
        let open = open.escape_unicode().to_string();
        let close = close.escape_unicode().to_string();
        let pattern = format!("{0}(?<content>[^{1}]*){1}", open, close);
        map(regex_capture(pattern, "content"), move |value: String| {
            snailquote::unescape(&value).unwrap()
        })
    }

    pub fn id_start() -> ParserHandle {
        regex(r"[a-zA-Z_]")
    }

    pub fn id_continue() -> ParserHandle {
        regex(r"[0-9a-zA-Z_]*")
    }

    pub fn id() -> ParserHandle {
        regex(r"[a-zA-Z_][0-9a-zA-Z_]*")
    }

    pub fn ws() -> ParserHandle {
        WhiteSpaceParser::default().into_handle()
    }

    pub fn ows() -> ParserHandle {
        OptionalWhiteSpaceParser::default().into_handle()
    }
}

#[derive(Clone)]
pub struct RegexParser {
    regex: regex::Regex,
    capture: Option<String>,
}

impl RegexParser {
    pub fn new(pattern: impl AsRef<str>) -> Self {
        let pattern = format!(r"^{}", pattern.as_ref());
        Self {
            regex: regex::Regex::new(&pattern).expect("Expected valid regex"),
            capture: None,
        }
    }

    pub fn new_capture(pattern: impl AsRef<str>, capture: impl ToString) -> Self {
        let pattern = format!(r"^{}", pattern.as_ref());
        Self {
            regex: regex::Regex::new(&pattern).expect("Expected valid regex"),
            capture: Some(capture.to_string()),
        }
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
        shorthand::{ows, regex, regex_capture, string, ws},
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
}
