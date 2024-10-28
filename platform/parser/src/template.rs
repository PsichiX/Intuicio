use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};
use regex::{Captures, Regex};

pub mod shorthand {
    use super::*;

    pub fn template(
        parser: ParserHandle,
        rule: impl ToString,
        content: impl ToString,
    ) -> ParserHandle {
        TemplateParser::new(parser, rule, content).into_handle()
    }
}

pub struct TemplateParser {
    parser: ParserHandle,
    rule: String,
    content: String,
}

impl TemplateParser {
    pub fn new(parser: ParserHandle, rule: impl ToString, content: impl ToString) -> Self {
        Self {
            parser,
            rule: rule.to_string(),
            content: content.to_string(),
        }
    }
}

impl Parser for TemplateParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let (input, result) = self.parser.parse(registry, input)?;
        let content = if let Some(value) = result.read::<String>() {
            self.content.replace("@{}@", &value)
        } else if let Some(list) = result.read::<Vec<ParserOutput>>() {
            Regex::new(r"@\{([^\}]*)\}\[([^\]@]*)\]\{([^\}]*)\}(\[(\d+)\])?@")
                .expect("Expected valid regex")
                .replace_all(&self.content, |caps: &Captures| -> String {
                    let prefix = caps.get(1).unwrap().as_str();
                    let delimiter = caps.get(2).unwrap().as_str();
                    let suffix = caps.get(3).unwrap().as_str();
                    let mut result = String::default();
                    if let Some(index) = caps.get(5) {
                        let index = index.as_str().parse::<usize>().unwrap();
                        result.push_str(prefix);
                        let item = list
                            .get(index)
                            .unwrap_or_else(|| {
                                panic!(
                                    "Template parsing result list has no item at {} index!",
                                    index
                                )
                            })
                            .read::<String>()
                            .unwrap_or_else(|| {
                                panic!("Template parsing result list item {} is not String!", index)
                            });
                        result.push_str(item.as_str());
                        result.push_str(suffix);
                    } else {
                        for (index, item) in list.iter().enumerate() {
                            if index > 0 {
                                result.push_str(delimiter);
                            }
                            result.push_str(prefix);
                            let item = item.read::<String>().unwrap_or_else(|| {
                                panic!("Template parsing result list item {} is not String!", index)
                            });
                            result.push_str(item.as_str());
                            result.push_str(suffix);
                        }
                    }
                    result
                })
                .to_string()
        } else {
            return Err("Template parsing result is not String or Vec<ParserOutput>!".into());
        };
        let (rest, result) = registry.parse(&self.rule, &content)?;
        if rest.is_empty() {
            Ok((input, result))
        } else {
            Err("Templating content parsing did not consumed all source!".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        shorthand::{inject, lit, map, number_int, prefix, seq_del, source, template, ws},
        template::TemplateParser,
        ParserOutput, ParserRegistry,
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
                template(source(number_int()), "value", "value:@{}@"),
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
                    "add",
                    "@{value:}[+]{}@",
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
                    "mul",
                    "value:@{}[]{}[0]@*value:@{}[]{}[1]@",
                ),
            );

        let (rest, result) = registry.parse("value", "value:42").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 42);

        let (rest, result) = registry.parse("add", "value:40+value:2").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 42);

        let (rest, result) = registry.parse("mul", "value:6*value:4").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 24);

        let (rest, result) = registry.parse("template_value", "42").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 42);

        let (rest, result) = registry.parse("template_add", "40 2").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 42);

        let (rest, result) = registry.parse("template_mul", "6 4").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<i32>().ok().unwrap(), 24);
    }
}
