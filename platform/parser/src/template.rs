use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};
use regex::{Captures, Regex};

pub mod shorthand {
    use super::*;

    pub fn template(
        parser: ParserHandle,
        rule: Option<String>,
        content: impl ToString,
    ) -> ParserHandle {
        TemplateParser::new(parser, rule, content).into_handle()
    }
}

thread_local! {
    static REGEX: Regex = Regex::new(r"@(>|<)\{([^\}]*)\}\[([^\]@]*)\]\{([^\}]*)\}(\[(\d+)\])?@").unwrap();
}

pub struct TemplateParser {
    parser: ParserHandle,
    rule: Option<String>,
    content: String,
}

impl TemplateParser {
    pub fn new(parser: ParserHandle, rule: Option<String>, content: impl ToString) -> Self {
        Self {
            parser,
            rule,
            content: content.to_string(),
        }
    }
}

impl Parser for TemplateParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let (input, result) = self.parser.parse(registry, input)?;
        let content =
            if let Some(value) = result.read::<String>() {
                self.content.replace("@{}@", &value)
            } else if let Some(list) = result.read::<Vec<ParserOutput>>() {
                REGEX.with(|regex| {
                    regex
                        .replace_all(&self.content, |caps: &Captures| -> String {
                            let ordering = caps.get(1).unwrap().as_str();
                            let prefix = caps.get(2).unwrap().as_str();
                            let delimiter = caps.get(3).unwrap().as_str();
                            let suffix = caps.get(4).unwrap().as_str();
                            let mut result = String::default();
                            if let Some(index) = caps.get(6) {
                                let index = index.as_str().parse::<usize>().unwrap();
                                result.push_str(prefix);
                                let item = list
                            .get(index)
                            .unwrap_or_else(|| {
                                panic!("Template parsing result list has no item at {index} index!")
                            })
                            .read::<String>()
                            .unwrap_or_else(|| {
                                panic!("Template parsing result list item {index} is not String!")
                            });
                                result.push_str(item.as_str());
                                result.push_str(suffix);
                            } else if ordering == ">" {
                                for (index, item) in list.iter().enumerate() {
                                    if index > 0 {
                                        result.push_str(delimiter);
                                    }
                                    result.push_str(prefix);
                                    let item = item.read::<String>().unwrap_or_else(|| {
                                panic!("Template parsing result list item {index} is not String!")
                            });
                                    result.push_str(item.as_str());
                                    result.push_str(suffix);
                                }
                            } else if ordering == "<" {
                                for (index, item) in list.iter().rev().enumerate() {
                                    if index > 0 {
                                        result.push_str(delimiter);
                                    }
                                    result.push_str(prefix);
                                    let item = item.read::<String>().unwrap_or_else(|| {
                                panic!("Template parsing result list item {index} is not String!")
                            });
                                    result.push_str(item.as_str());
                                    result.push_str(suffix);
                                }
                            }
                            result
                        })
                        .to_string()
                })
            } else {
                return Err("Template parsing result is not String or Vec<ParserOutput>!".into());
            };
        if let Some(rule) = self.rule.as_ref() {
            let (rest, result) = registry.parse(rule, &content)?;
            if rest.is_empty() {
                Ok((input, result))
            } else {
                Err("Templating content parsing did not consumed all source!".into())
            }
        } else {
            Ok((input, ParserOutput::new(content).ok().unwrap()))
        }
    }

    fn extend(&self, parser: ParserHandle) {
        self.parser.extend(parser);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserOutput, ParserRegistry,
        shorthand::{inject, lit, map, number_int, prefix, seq_del, source, template, ws},
        template::TemplateParser,
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_template() {
        is_async::<TemplateParser>();

        let registry = ParserRegistry::default()
            .with_parser(
                "value",
                map(prefix(number_int(), lit("value:")), |value: String| {
                    value.parse::<i32>().unwrap()
                }),
            )
            .with_parser(
                "add",
                map(
                    seq_del(lit("+"), [inject("value"), inject("value")]),
                    |mut values: Vec<ParserOutput>| {
                        let b = values.remove(1).consume::<i32>().ok().unwrap();
                        let a = values.remove(0).consume::<i32>().ok().unwrap();
                        a + b
                    },
                ),
            )
            .with_parser(
                "sub",
                map(
                    seq_del(lit("-"), [inject("value"), inject("value")]),
                    |mut values: Vec<ParserOutput>| {
                        let b = values.remove(1).consume::<i32>().ok().unwrap();
                        let a = values.remove(0).consume::<i32>().ok().unwrap();
                        a - b
                    },
                ),
            )
            .with_parser(
                "mul",
                map(
                    seq_del(lit("*"), [inject("value"), inject("value")]),
                    |mut values: Vec<ParserOutput>| {
                        let b = values.remove(1).consume::<i32>().ok().unwrap();
                        let a = values.remove(0).consume::<i32>().ok().unwrap();
                        a * b
                    },
                ),
            )
            .with_parser(
                "template_value",
                template(source(number_int()), Some("value".to_owned()), "value:@{}@"),
            )
            .with_parser(
                "template_add",
                template(
                    seq_del(
                        ws(),
                        [
                            source(inject("template_value")),
                            source(inject("template_value")),
                        ],
                    ),
                    Some("add".to_owned()),
                    "@>{value:}[+]{}@",
                ),
            )
            .with_parser(
                "template_sub",
                template(
                    seq_del(
                        ws(),
                        [
                            source(inject("template_value")),
                            source(inject("template_value")),
                        ],
                    ),
                    Some("sub".to_owned()),
                    "@<{value:}[-]{}@",
                ),
            )
            .with_parser(
                "template_mul",
                template(
                    seq_del(
                        ws(),
                        [
                            source(inject("template_value")),
                            source(inject("template_value")),
                        ],
                    ),
                    Some("mul".to_owned()),
                    "value:@>{}[]{}[0]@*value:@>{}[]{}[1]@",
                ),
            )
            .with_parser(
                "template_output",
                template(source(inject("template_value")), None, "#@{}@"),
            );

        let (rest, result) = registry.parse("value", "value:42").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 42);

        let (rest, result) = registry.parse("add", "value:40+value:2").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 42);

        let (rest, result) = registry.parse("sub", "value:40-value:2").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 38);

        let (rest, result) = registry.parse("mul", "value:6*value:4").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 24);

        let (rest, result) = registry.parse("template_value", "42").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 42);

        let (rest, result) = registry.parse("template_add", "40 2").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 42);

        let (rest, result) = registry.parse("template_sub", "2 40").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 38);

        let (rest, result) = registry.parse("template_mul", "6 4").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 24);

        let (rest, result) = registry.parse("template_output", "42").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<String>().ok().unwrap(), "#42");
    }
}
