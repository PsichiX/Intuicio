use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};

pub mod shorthand {
    use super::*;

    pub fn list(item: ParserHandle, delimiter: ParserHandle, permissive: bool) -> ParserHandle {
        ListParser::new(item, delimiter, permissive).into_handle()
    }
}

#[derive(Clone)]
pub struct ListParser {
    item: ParserHandle,
    delimiter: ParserHandle,
    permissive: bool,
}

impl ListParser {
    pub fn new(item: ParserHandle, delimiter: ParserHandle, permissive: bool) -> Self {
        Self {
            item,
            delimiter,
            permissive,
        }
    }
}

impl Parser for ListParser {
    fn parse<'a>(&self, registry: &ParserRegistry, mut input: &'a str) -> ParseResult<'a> {
        let mut result = vec![];
        if let Ok((new_input, value)) = self.item.parse(registry, input) {
            input = new_input;
            result.push(value);
            while let Ok((new_input, _)) = self.delimiter.parse(registry, input) {
                match self.item.parse(registry, new_input) {
                    Ok((new_input, value)) => {
                        input = new_input;
                        result.push(value);
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
        list::ListParser,
        shorthand::{alt, list, lit, ows},
        ParserOutput, ParserRegistry,
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
