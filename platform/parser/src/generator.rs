use crate::{
    pratt::PrattParserAssociativity,
    shorthand::{
        alpha, alpha_low, alpha_up, alphanum, alt, any, debug, digit, digit_hex, dyn_inspect,
        dyn_map, dyn_map_err, dyn_pratt, ext_depth, ext_exchange, ext_variants, ext_wrap, id,
        id_continue, id_start, ignore, inject, list, lit, map, map_err, nl, not, number_float,
        number_int, number_int_pos, oc, omap, oom, opt, pred, prefix, regex, rep, seq, seq_del,
        slot_empty, source, string, suffix, template, word, zom, DynamicPrattParserRule,
    },
    ParseResult, Parser, ParserExt, ParserHandle, ParserNoValue, ParserOutput, ParserRegistry,
};
use std::{collections::HashMap, error::Error, sync::RwLock};

#[derive(Default)]
struct SlotsExtension {
    slots: RwLock<HashMap<String, ParserHandle>>,
}

impl SlotsExtension {
    fn make(&self, name: impl ToString) -> Option<ParserHandle> {
        let parser = slot_empty();
        self.slots
            .write()
            .ok()?
            .insert(name.to_string(), parser.clone());
        Some(parser)
    }

    fn take(&self, name: &str) -> Option<ParserHandle> {
        self.slots.write().ok()?.remove(name)
    }
}

struct SlotExtensionSlotParser(ParserHandle);

impl Parser for SlotExtensionSlotParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let (input, value) = self.0.parse(registry, input)?;
        let name = value.consume::<String>().ok().unwrap();
        if let Some(result) = registry
            .extension::<SlotsExtension>()
            .expect("Could not access SlotExtension")
            .make(&name)
        {
            Ok((input, ParserOutput::new(result).ok().unwrap()))
        } else {
            Err(format!("Could not make `{}` slot parser", name).into())
        }
    }
}

struct SlotExtensionExtWrapParser(ParserHandle);

impl Parser for SlotExtensionExtWrapParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let (input, value) = self.0.parse(registry, input)?;
        let mut values = value.consume::<Vec<ParserOutput>>().ok().unwrap();
        let item = values.remove(1).consume::<ParserHandle>().ok().unwrap();
        let name = values.remove(0).consume::<String>().ok().unwrap();
        if let Some(slot) = registry
            .extension::<SlotsExtension>()
            .expect("Could not access SlotExtension")
            .take(&name)
        {
            Ok((input, ParserOutput::new(ext_wrap(item, slot)).ok().unwrap()))
        } else {
            Err(format!("Could not take `{}` slot parser", name).into())
        }
    }
}

pub struct Generator {
    parsers: Vec<(String, ParserHandle, Option<String>)>,
}

impl Generator {
    pub fn new(grammar: &str) -> Result<Self, Box<dyn Error>> {
        let registry = Self::registry();
        Ok(Self {
            parsers: main()
                .parse(&registry, grammar)?
                .1
                .consume::<Vec<(String, ParserHandle, Option<String>)>>()
                .ok()
                .unwrap(),
        })
    }

    pub fn install(&self, registry: &mut ParserRegistry) -> Result<(), Box<dyn Error>> {
        for (id, parser, extender) in &self.parsers {
            registry.add_parser(id, parser.clone());
            if let Some(id) = extender.as_ref() {
                registry.extend(id, parser.clone())?;
            }
        }
        Ok(())
    }

    pub fn parser(&self, id: &str) -> Option<ParserHandle> {
        self.parsers
            .iter()
            .find_map(|(k, v, _)| if k == id { Some(v.clone()) } else { None })
    }

    fn registry() -> ParserRegistry {
        ParserRegistry::default()
            .with_extension(SlotsExtension::default())
            .with_parser("item", item())
            .with_parser("debug", parser_debug())
            .with_parser("source", parser_source())
            .with_parser("ext_exchange", parser_ext_exchange())
            .with_parser("ext_depth", parser_ext_depth())
            .with_parser("ext_variants", parser_ext_variants())
            .with_parser("ext_wrap", parser_ext_wrap())
            .with_parser("inspect", parser_inspect())
            .with_parser("map", parser_map())
            .with_parser("map_err", parser_map_err())
            .with_parser("pratt", parser_pratt())
            .with_parser("alt", parser_alt())
            .with_parser("seq", parser_seq())
            .with_parser("seq_del", parser_seq_del())
            .with_parser("zom", parser_zom())
            .with_parser("oom", parser_oom())
            .with_parser("not", parser_not())
            .with_parser("opt", parser_opt())
            .with_parser("pred", parser_pred())
            .with_parser("slot", parser_slot())
            .with_parser("rep", parser_rep())
            .with_parser("inject", parser_inject())
            .with_parser("lit", parser_lit())
            .with_parser("regex", parser_regex())
            .with_parser("template", parser_template())
            .with_parser("oc", parser_oc())
            .with_parser("prefix", parser_prefix())
            .with_parser("suffix", parser_suffix())
            .with_parser("string", parser_string())
            .with_parser("list", parser_list())
            .with_parser("any", parser_any())
            .with_parser("nl", parser_nl())
            .with_parser("digit_hex", parser_digit_hex())
            .with_parser("digit", parser_digit())
            .with_parser("number_int_pos", parser_number_int_pos())
            .with_parser("number_int", parser_number_int())
            .with_parser("number_float", parser_number_float())
            .with_parser("alphanum", parser_alphanum())
            .with_parser("alpha_low", parser_alpha_low())
            .with_parser("alpha_up", parser_alpha_up())
            .with_parser("alpha", parser_alpha())
            .with_parser("word", parser_word())
            .with_parser("id_start", parser_id_start())
            .with_parser("id_continue", parser_id_continue())
            .with_parser("id", parser_id())
            .with_parser("ows", parser_ows())
            .with_parser("ws", parser_ws())
            .with_parser("ignore", parser_ignore())
    }
}

fn main() -> ParserHandle {
    map(
        oc(list(rule(), ws(), true), ows(), ows()),
        |values: Vec<ParserOutput>| {
            values
                .into_iter()
                .map(|value| {
                    value
                        .consume::<(String, ParserHandle, Option<String>)>()
                        .ok()
                        .unwrap()
                })
                .collect::<Vec<_>>()
        },
    )
}

fn identifier() -> ParserHandle {
    alt([string("`", "`"), id()])
}

fn boolean() -> ParserHandle {
    map(alt([lit("true"), lit("false")]), |value: String| {
        value.parse::<bool>().unwrap()
    })
}

fn rule() -> ParserHandle {
    map(
        seq_del(
            ows(),
            [
                identifier(),
                opt(prefix(prefix(identifier(), ows()), lit("->"))),
                lit("=>"),
                inject("item"),
            ],
        ),
        |mut values: Vec<ParserOutput>| {
            let parser = values.remove(3).consume::<ParserHandle>().ok().unwrap();
            let extends = values.remove(1).consume::<String>().ok();
            let id = values.remove(0).consume::<String>().ok().unwrap();
            (id, parser, extends)
        },
    )
}

fn comment() -> ParserHandle {
    map(
        regex(r"(\s*/\*[^\*/]+\*/\s*|\s*//[^\r\n]+[\r\n]\s*)+"),
        |_: String| ParserNoValue,
    )
}

fn ws() -> ParserHandle {
    alt([comment(), crate::shorthand::ws()])
}

fn ows() -> ParserHandle {
    alt([comment(), crate::shorthand::ows()])
}

fn item() -> ParserHandle {
    alt([
        inject("debug"),
        inject("source"),
        inject("ext_exchange"),
        inject("ext_depth"),
        inject("ext_variants"),
        inject("ext_wrap"),
        inject("inspect"),
        inject("map"),
        inject("map_err"),
        inject("pratt"),
        inject("alt"),
        inject("seq"),
        inject("seq_del"),
        inject("zom"),
        inject("oom"),
        inject("not"),
        inject("opt"),
        inject("pred"),
        inject("slot"),
        inject("rep"),
        inject("inject"),
        inject("lit"),
        inject("regex"),
        inject("template"),
        inject("oc"),
        inject("prefix"),
        inject("suffix"),
        inject("string"),
        inject("list"),
        inject("any"),
        inject("nl"),
        inject("digit_hex"),
        inject("digit"),
        inject("number_int_pos"),
        inject("number_int"),
        inject("number_float"),
        inject("alphanum"),
        inject("alpha_low"),
        inject("alpha_up"),
        inject("alpha"),
        inject("word"),
        inject("id_start"),
        inject("id_continue"),
        inject("id"),
        inject("ows"),
        inject("ws"),
        inject("ignore"),
    ])
}

fn parser_list() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(ws(), [inject("item"), inject("item"), boolean()]),
                suffix(lit("{"), ows()),
                prefix(lit("}"), ows()),
            ),
            |mut values: Vec<ParserOutput>| {
                let permissive = values.remove(2).consume::<bool>().ok().unwrap();
                let delimiter = values.remove(1).consume::<ParserHandle>().ok().unwrap();
                let item = values.remove(0).consume::<ParserHandle>().ok().unwrap();
                list(item, delimiter, permissive)
            },
        ),
        |_| "Expected list".into(),
    )
}

fn parser_debug() -> ParserHandle {
    map_err(
        map(
            prefix(seq([string("`", "`"), inject("item")]), lit("@")),
            |mut values: Vec<ParserOutput>| {
                let item = values.remove(1).consume::<ParserHandle>().ok().unwrap();
                let id = values.remove(0).consume::<String>().ok().unwrap();
                debug(id, item)
            },
        ),
        |_| "Expected debug".into(),
    )
}

fn parser_source() -> ParserHandle {
    map_err(
        map(prefix(inject("item"), lit("=")), |value: ParserHandle| {
            source(value)
        }),
        |_| "Expected source".into(),
    )
}

fn parser_ext_exchange() -> ParserHandle {
    map_err(
        map(
            oc(
                inject("item"),
                seq_del(ows(), [lit("#"), lit("exchange"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |value: ParserHandle| ext_exchange(value),
        ),
        |_| "Expected extendable exchange".into(),
    )
}

fn parser_ext_depth() -> ParserHandle {
    map_err(
        map(
            oc(
                inject("item"),
                seq_del(ows(), [lit("#"), lit("depth"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |value: ParserHandle| ext_depth(value),
        ),
        |_| "Expected extendable depth".into(),
    )
}

fn parser_ext_variants() -> ParserHandle {
    map_err(
        omap(
            seq_del(ows(), [lit("#"), lit("variants"), lit("{"), lit("}")]),
            |_| ParserOutput::new(ext_variants()).ok().unwrap(),
        ),
        |_| "Expected extendable variants".into(),
    )
}

fn parser_ext_wrap() -> ParserHandle {
    map_err(
        SlotExtensionExtWrapParser(oc(
            seq_del(ws(), [identifier(), inject("item")]),
            seq_del(ows(), [lit("#"), lit("wrapper"), suffix(lit("{"), ows())]),
            prefix(lit("}"), ows()),
        ))
        .into_handle(),
        |_| "Expected extendable wrapper".into(),
    )
}

fn parser_inspect() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(ws(), [inject("item"), string("\"", "\"")]),
                seq_del(ows(), [lit("%"), lit("inspect"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |mut values: Vec<ParserOutput>| {
                let callback = values.remove(1).consume::<String>().ok().unwrap();
                let item = values.remove(0).consume::<ParserHandle>().ok().unwrap();
                dyn_inspect(item, callback)
            },
        ),
        |_| "Expected inspection".into(),
    )
}

fn parser_map() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(ws(), [inject("item"), string("\"", "\"")]),
                seq_del(ows(), [lit("%"), lit("map"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |mut values: Vec<ParserOutput>| {
                let callback = values.remove(1).consume::<String>().ok().unwrap();
                let item = values.remove(0).consume::<ParserHandle>().ok().unwrap();
                dyn_map(item, callback)
            },
        ),
        |_| "Expected mapping".into(),
    )
}

fn parser_map_err() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(ws(), [inject("item"), string("\"", "\"")]),
                seq_del(ows(), [lit("%"), lit("maperr"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |mut values: Vec<ParserOutput>| {
                let callback = values.remove(1).consume::<String>().ok().unwrap();
                let item = values.remove(0).consume::<ParserHandle>().ok().unwrap();
                dyn_map_err(item, callback)
            },
        ),
        |_| "Expected error mapping".into(),
    )
}

fn pratt_rule_prefix() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(
                    ws(),
                    [lit("prefix"), string("\"", "\""), string("\"", "\"")],
                ),
                lit("<"),
                lit(">"),
            ),
            |mut values: Vec<ParserOutput>| {
                let transformer_function_name = values.remove(2).consume::<String>().ok().unwrap();
                let operator_function_name = values.remove(1).consume::<String>().ok().unwrap();
                DynamicPrattParserRule::Prefix {
                    operator_function_name,
                    transformer_function_name,
                }
            },
        ),
        |_| "Expected Pratt prefix rule".into(),
    )
}

fn pratt_rule_prefix_op() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(
                    ws(),
                    [
                        lit("prefix"),
                        lit("op"),
                        string("\"", "\""),
                        string("\"", "\""),
                    ],
                ),
                lit("<"),
                lit(">"),
            ),
            |mut values: Vec<ParserOutput>| {
                let transformer_function_name = values.remove(3).consume::<String>().ok().unwrap();
                let operator = values.remove(2).consume::<String>().ok().unwrap();
                DynamicPrattParserRule::PrefixOp {
                    operator,
                    transformer_function_name,
                }
            },
        ),
        |_| "Expected Pratt prefix rule".into(),
    )
}

fn pratt_rule_postfix() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(
                    ws(),
                    [lit("postfix"), string("\"", "\""), string("\"", "\"")],
                ),
                lit("<"),
                lit(">"),
            ),
            |mut values: Vec<ParserOutput>| {
                let transformer_function_name = values.remove(2).consume::<String>().ok().unwrap();
                let operator_function_name = values.remove(1).consume::<String>().ok().unwrap();
                DynamicPrattParserRule::Postfix {
                    operator_function_name,
                    transformer_function_name,
                }
            },
        ),
        |_| "Expected Pratt postfix rule".into(),
    )
}

fn pratt_rule_postfix_op() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(
                    ws(),
                    [
                        lit("postfix"),
                        lit("op"),
                        string("\"", "\""),
                        string("\"", "\""),
                    ],
                ),
                lit("<"),
                lit(">"),
            ),
            |mut values: Vec<ParserOutput>| {
                let transformer_function_name = values.remove(3).consume::<String>().ok().unwrap();
                let operator = values.remove(2).consume::<String>().ok().unwrap();
                DynamicPrattParserRule::PostfixOp {
                    operator,
                    transformer_function_name,
                }
            },
        ),
        |_| "Expected Pratt postfix rule".into(),
    )
}

fn pratt_rule_infix() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(
                    ws(),
                    [
                        lit("infix"),
                        string("\"", "\""),
                        string("\"", "\""),
                        alt([lit("left"), lit("right")]),
                    ],
                ),
                lit("<"),
                lit(">"),
            ),
            |mut values: Vec<ParserOutput>| {
                let associativity = values.remove(3).consume::<String>().ok().unwrap();
                let transformer = values.remove(2).consume::<String>().ok().unwrap();
                let operator = values.remove(1).consume::<String>().ok().unwrap();
                DynamicPrattParserRule::Infix {
                    operator_function_name: operator,
                    transformer_function_name: transformer,
                    associativity: match associativity.as_str() {
                        "left" => PrattParserAssociativity::Left,
                        "right" => PrattParserAssociativity::Right,
                        _ => unreachable!(),
                    },
                }
            },
        ),
        |_| "Expected Pratt infix rule".into(),
    )
}

fn pratt_rule_infix_op() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(
                    ws(),
                    [
                        lit("infix"),
                        lit("op"),
                        string("\"", "\""),
                        string("\"", "\""),
                        alt([lit("left"), lit("right")]),
                    ],
                ),
                lit("<"),
                lit(">"),
            ),
            |mut values: Vec<ParserOutput>| {
                let associativity = values.remove(4).consume::<String>().ok().unwrap();
                let transformer_function_name = values.remove(3).consume::<String>().ok().unwrap();
                let operator = values.remove(2).consume::<String>().ok().unwrap();
                DynamicPrattParserRule::InfixOp {
                    operator,
                    transformer_function_name,
                    associativity: match associativity.as_str() {
                        "left" => PrattParserAssociativity::Left,
                        "right" => PrattParserAssociativity::Right,
                        _ => unreachable!(),
                    },
                }
            },
        ),
        |_| "Expected Pratt infix rule".into(),
    )
}

fn pratt_rule_set() -> ParserHandle {
    map_err(
        map(
            oc(
                list(
                    alt([
                        pratt_rule_prefix_op(),
                        pratt_rule_prefix(),
                        pratt_rule_postfix_op(),
                        pratt_rule_postfix(),
                        pratt_rule_infix_op(),
                        pratt_rule_infix(),
                    ]),
                    ws(),
                    false,
                ),
                suffix(lit("["), ows()),
                prefix(lit("]"), ows()),
            ),
            |values: Vec<ParserOutput>| {
                values
                    .into_iter()
                    .map(|value| value.consume::<DynamicPrattParserRule>().ok().unwrap())
                    .collect::<Vec<_>>()
            },
        ),
        |_| "Expected Pratt rule set".into(),
    )
}

fn parser_pratt() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(
                    ws(),
                    [
                        inject("item"),
                        lit("->"),
                        list(pratt_rule_set(), ws(), false),
                    ],
                ),
                seq_del(ows(), [lit("%"), lit("pratt"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |mut values: Vec<ParserOutput>| {
                let rules = values
                    .remove(2)
                    .consume::<Vec<ParserOutput>>()
                    .ok()
                    .unwrap();
                let rules = rules
                    .into_iter()
                    .map(|value| value.consume::<Vec<DynamicPrattParserRule>>().ok().unwrap())
                    .collect::<Vec<_>>();
                let tokenizer_parser = values.remove(0).consume::<ParserHandle>().ok().unwrap();
                dyn_pratt(tokenizer_parser, rules)
            },
        ),
        |_| "Expected error mapping".into(),
    )
}

fn parser_alt() -> ParserHandle {
    map_err(
        map(
            oc(
                list(inject("item"), ws(), false),
                suffix(lit("["), ows()),
                prefix(lit("]"), ows()),
            ),
            |values: Vec<ParserOutput>| {
                let parsers = values
                    .into_iter()
                    .map(|value| value.consume::<ParserHandle>().ok().unwrap())
                    .collect::<Vec<_>>();
                alt(parsers)
            },
        ),
        |_| "Expected alternations".into(),
    )
}

fn parser_seq() -> ParserHandle {
    map_err(
        map(
            oc(
                list(inject("item"), ws(), false),
                suffix(lit("("), ows()),
                prefix(lit(")"), ows()),
            ),
            |values: Vec<ParserOutput>| {
                let parsers = values
                    .into_iter()
                    .map(|value| value.consume::<ParserHandle>().ok().unwrap())
                    .collect::<Vec<_>>();
                seq(parsers)
            },
        ),
        |_| "Expected sequence".into(),
    )
}

fn parser_seq_del() -> ParserHandle {
    map_err(
        map(
            seq_del(
                ows(),
                [
                    oc(
                        inject("item"),
                        suffix(lit("|"), ows()),
                        prefix(lit("|"), ows()),
                    ),
                    oc(
                        list(inject("item"), ws(), false),
                        suffix(lit("("), ows()),
                        prefix(lit(")"), ows()),
                    ),
                ],
            ),
            |mut values: Vec<ParserOutput>| {
                let parsers = values
                    .remove(1)
                    .consume::<Vec<ParserOutput>>()
                    .ok()
                    .unwrap();
                let delimiter = values.remove(0).consume::<ParserHandle>().ok().unwrap();
                let parsers = parsers
                    .into_iter()
                    .map(|value| value.consume::<ParserHandle>().ok().unwrap())
                    .collect::<Vec<_>>();
                seq_del(delimiter, parsers)
            },
        ),
        |_| "Expected delimited sequence".into(),
    )
}

fn parser_zom() -> ParserHandle {
    map_err(
        map(prefix(inject("item"), lit("*")), |value: ParserHandle| {
            zom(value)
        }),
        |_| "Expected zero or more".into(),
    )
}

fn parser_oom() -> ParserHandle {
    map_err(
        map(prefix(inject("item"), lit("+")), |value: ParserHandle| {
            oom(value)
        }),
        |_| "Expected one or more".into(),
    )
}

fn parser_not() -> ParserHandle {
    map_err(
        map(prefix(inject("item"), lit("!")), |value: ParserHandle| {
            not(value)
        }),
        |_| "Expected negation".into(),
    )
}

fn parser_opt() -> ParserHandle {
    map_err(
        map(prefix(inject("item"), lit("?")), |value: ParserHandle| {
            opt(value)
        }),
        |_| "Expected optional".into(),
    )
}

fn parser_pred() -> ParserHandle {
    map_err(
        map(prefix(inject("item"), lit("^")), |value: ParserHandle| {
            pred(value)
        }),
        |_| "Expected prediction".into(),
    )
}

fn parser_slot() -> ParserHandle {
    map_err(
        SlotExtensionSlotParser(oc(identifier(), lit("<"), lit(">"))).into_handle(),
        |_| "Expected slot".into(),
    )
}

fn parser_rep() -> ParserHandle {
    map_err(
        map(
            seq([number_int_pos(), inject("item")]),
            |mut values: Vec<ParserOutput>| {
                let parser = values.remove(1).consume::<ParserHandle>().ok().unwrap();
                let occurrences = values
                    .remove(0)
                    .consume::<String>()
                    .ok()
                    .unwrap()
                    .parse()
                    .unwrap();
                rep(parser, occurrences)
            },
        ),
        |_| "Expected repetition".into(),
    )
}

fn parser_inject() -> ParserHandle {
    map_err(
        map(prefix(identifier(), lit("$")), |value: String| {
            inject(value)
        }),
        |_| "Expected injection".into(),
    )
}

fn parser_lit() -> ParserHandle {
    map_err(map(string("\"", "\""), |value: String| lit(value)), |_| {
        "Expected literal".into()
    })
}

fn parser_regex() -> ParserHandle {
    map_err(
        map(string("~~~(", ")~~~"), |value: String| regex(value)),
        |_| "Expected regex".into(),
    )
}

fn parser_template() -> ParserHandle {
    map_err(
        map(
            oc(
                seq([
                    inject("item"),
                    opt(prefix(string("\"", "\""), ws())),
                    prefix(string("```", "```"), ws()),
                ]),
                seq_del(ows(), [lit("template"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |mut values: Vec<ParserOutput>| {
                let content = values.remove(2).consume::<String>().ok().unwrap();
                let rule = values.remove(1).consume::<String>().ok();
                let item = values.remove(0).consume::<ParserHandle>().ok().unwrap();
                template(item, rule, content)
            },
        ),
        |_| "Expected template".into(),
    )
}

fn parser_oc() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(ws(), [inject("item"), inject("item"), inject("item")]),
                seq_del(ows(), [lit("oc"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |mut values: Vec<ParserOutput>| {
                let close = values.remove(2).consume::<ParserHandle>().ok().unwrap();
                let open = values.remove(1).consume::<ParserHandle>().ok().unwrap();
                let item = values.remove(0).consume::<ParserHandle>().ok().unwrap();
                oc(item, open, close)
            },
        ),
        |_| "Expected open-close".into(),
    )
}

fn parser_prefix() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(ws(), [inject("item"), inject("item")]),
                seq_del(ows(), [lit("prefix"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |mut values: Vec<ParserOutput>| {
                let before = values.remove(1).consume::<ParserHandle>().ok().unwrap();
                let item = values.remove(0).consume::<ParserHandle>().ok().unwrap();
                prefix(item, before)
            },
        ),
        |_| "Expected prefix".into(),
    )
}

fn parser_suffix() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(ws(), [inject("item"), inject("item")]),
                seq_del(ows(), [lit("suffix"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |mut values: Vec<ParserOutput>| {
                let after = values.remove(1).consume::<ParserHandle>().ok().unwrap();
                let item = values.remove(0).consume::<ParserHandle>().ok().unwrap();
                suffix(item, after)
            },
        ),
        |_| "Expected suffix".into(),
    )
}

fn parser_string() -> ParserHandle {
    map_err(
        map(
            oc(
                seq_del(ws(), [string("\"", "\""), string("\"", "\"")]),
                seq_del(ows(), [lit("string"), suffix(lit("{"), ows())]),
                prefix(lit("}"), ows()),
            ),
            |mut values: Vec<ParserOutput>| {
                let close = values.remove(1).consume::<String>().ok().unwrap();
                let open = values.remove(0).consume::<String>().ok().unwrap();
                string(&open, &close)
            },
        ),
        |_| "Expected string".into(),
    )
}

fn parser_any() -> ParserHandle {
    map_err(map(lit("any"), |_: String| any()), |_| {
        "Expected any".into()
    })
}

fn parser_nl() -> ParserHandle {
    map_err(map(lit("nl"), |_: String| nl()), |_| {
        "Expected new line".into()
    })
}

fn parser_digit_hex() -> ParserHandle {
    map_err(map(lit("digit_hex"), |_: String| digit_hex()), |_| {
        "Expected HEX digit".into()
    })
}

fn parser_digit() -> ParserHandle {
    map_err(map(lit("digit"), |_: String| digit()), |_| {
        "Expected digit".into()
    })
}

fn parser_number_int_pos() -> ParserHandle {
    map_err(
        map(lit("number_int_pos"), |_: String| number_int_pos()),
        |_| "Expected positive integer number".into(),
    )
}

fn parser_number_int() -> ParserHandle {
    map_err(map(lit("number_int"), |_: String| number_int()), |_| {
        "Expected integer number".into()
    })
}

fn parser_number_float() -> ParserHandle {
    map_err(map(lit("number_float"), |_: String| number_float()), |_| {
        "Expected float number".into()
    })
}

fn parser_alphanum() -> ParserHandle {
    map_err(map(lit("alphanum"), |_: String| alphanum()), |_| {
        "Expected alphanumeric character".into()
    })
}

fn parser_alpha_low() -> ParserHandle {
    map_err(map(lit("alpha_low"), |_: String| alpha_low()), |_| {
        "Expected lowercase alphabetic character".into()
    })
}

fn parser_alpha_up() -> ParserHandle {
    map_err(map(lit("alpha_up"), |_: String| alpha_up()), |_| {
        "Expected uppercase alphabetic character".into()
    })
}

fn parser_alpha() -> ParserHandle {
    map_err(map(lit("alpha"), |_: String| alpha()), |_| {
        "Expected alphabetic character".into()
    })
}

fn parser_word() -> ParserHandle {
    map_err(map(lit("word"), |_: String| word()), |_| {
        "Expected word".into()
    })
}

fn parser_id_start() -> ParserHandle {
    map_err(map(lit("id_start"), |_: String| id_start()), |_| {
        "Expected id start".into()
    })
}

fn parser_id_continue() -> ParserHandle {
    map_err(map(lit("id_continue"), |_: String| id_continue()), |_| {
        "Expected id continue".into()
    })
}

fn parser_id() -> ParserHandle {
    map_err(map(lit("id"), |_: String| id()), |_| "Expected id".into())
}

fn parser_ows() -> ParserHandle {
    map_err(map(lit("ows"), |_: String| crate::shorthand::ows()), |_| {
        "Expected optional whitespaces".into()
    })
}

fn parser_ws() -> ParserHandle {
    map_err(map(lit("ws"), |_: String| crate::shorthand::ws()), |_| {
        "Expected whitespaces".into()
    })
}

fn parser_ignore() -> ParserHandle {
    map_err(map(lit("ignore"), |_: String| ignore()), |_| {
        "Expected ignored".into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamic::DynamicExtensionBuilder;
    use intuicio_core::transformer::*;
    use intuicio_derive::intuicio_function;

    #[test]
    fn test_parsers() {
        let registry = Generator::registry();

        let (rest, result) = main()
            .parse(
                &registry,
                "//foo => any\r\nlist => {\"foo\" ws true}\r\n/*bar => any*/",
            )
            .unwrap();
        assert_eq!(rest, "");
        let result = result
            .consume::<Vec<(String, ParserHandle, Option<String>)>>()
            .ok()
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0.as_str(), "list");

        assert_eq!(comment().parse(&registry, "//foo\r\n").unwrap().0, "");
        assert_eq!(comment().parse(&registry, "/*bar*/").unwrap().0, "");
        assert_eq!(
            comment()
                .parse(&registry, "//macro => @main:(\r\n")
                .unwrap()
                .0,
            ""
        );

        assert_eq!(
            parser_string()
                .parse(&registry, "string{\"(\" \")\"}")
                .unwrap()
                .0,
            ""
        );

        let (rest, result) = parser_template()
            .parse(&registry, "template{\"foo\" ```@{}@```}")
            .unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            result
                .consume::<ParserHandle>()
                .ok()
                .unwrap()
                .parse(&registry, "foo")
                .unwrap()
                .1
                .consume::<String>()
                .ok()
                .unwrap()
                .as_str(),
            "foo"
        );

        let (rest, result) = parser_ext_wrap()
            .parse(&registry, "#wrapper{inner <inner>}")
            .unwrap();
        assert_eq!(rest, "");
        let parser = result.consume::<ParserHandle>().ok().unwrap();
        let registry = ParserRegistry::default();
        assert!(parser
            .parse(&registry, "foo")
            .unwrap()
            .1
            .is::<ParserNoValue>());
        parser.extend(lit("foo"));
        assert!(parser.parse(&registry, "foo").unwrap().1.is::<String>());
    }

    #[test]
    fn test_generator() {
        let grammar = std::fs::read_to_string("./resources/grammar.txt").unwrap();
        let generator = Generator::new(&grammar).unwrap();
        assert_eq!(
            generator
                .parsers
                .iter()
                .map(|(k, _, _)| k.as_str())
                .collect::<Vec<_>>(),
            vec![
                "debug",
                "source",
                "ext_exchange",
                "ext_depth",
                "ext_variants",
                "ext_wrap",
                "inspect",
                "map",
                "map_err",
                "pratt",
                "alt",
                "seq",
                "seq_del",
                "zom",
                "oom",
                "not",
                "opt",
                "pred",
                "slot",
                "rep",
                "inject",
                "lit",
                "regex",
                "template_value",
                "template_add",
                "template_mul",
                "template_output",
                "oc",
                "prefix",
                "suffix",
                "string",
                "list",
                "any",
                "nl",
                "digit",
                "digit_hex",
                "number_int_pos",
                "number_int",
                "number_float",
                "alphanum",
                "alpha_low",
                "alpha_up",
                "alpha",
                "word",
                "id_start",
                "id_continue",
                "id",
                "ows",
                "ws",
                "ignore",
                "bar",
            ]
        );

        let mut registry = ParserRegistry::default()
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
            );
        generator.install(&mut registry).unwrap();

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

    #[intuicio_function(transformer = "DynamicManagedValueTransformer")]
    fn map_value(value: String) -> f32 {
        value.parse().unwrap()
    }

    #[intuicio_function(transformer = "DynamicManagedValueTransformer")]
    fn map_value_error(_error: Box<dyn Error>) -> Box<dyn Error> {
        "Expected value".into()
    }

    #[intuicio_function(transformer = "DynamicManagedValueTransformer")]
    fn map_op_add(mut value: Vec<ParserOutput>) -> f32 {
        let b = value.remove(2).consume::<f32>().ok().unwrap();
        let a = value.remove(1).consume::<f32>().ok().unwrap();
        a + b
    }

    #[intuicio_function(transformer = "DynamicManagedValueTransformer")]
    fn map_op_sub(mut value: Vec<ParserOutput>) -> f32 {
        let b = value.remove(2).consume::<f32>().ok().unwrap();
        let a = value.remove(1).consume::<f32>().ok().unwrap();
        a - b
    }

    #[intuicio_function(transformer = "DynamicManagedValueTransformer")]
    fn map_op_mul(mut value: Vec<ParserOutput>) -> f32 {
        let b = value.remove(2).consume::<f32>().ok().unwrap();
        let a = value.remove(1).consume::<f32>().ok().unwrap();
        a * b
    }

    #[intuicio_function(transformer = "DynamicManagedValueTransformer")]
    fn map_op_div(mut value: Vec<ParserOutput>) -> f32 {
        let b = value.remove(2).consume::<f32>().ok().unwrap();
        let a = value.remove(1).consume::<f32>().ok().unwrap();
        a / b
    }

    #[test]
    fn test_calculator() {
        let grammar = std::fs::read_to_string("./resources/calculator.txt").unwrap();
        let generator = Generator::new(&grammar).unwrap();
        assert_eq!(
            generator
                .parsers
                .iter()
                .map(|(k, _, _)| k.as_str())
                .collect::<Vec<_>>(),
            vec!["value", "op_add", "op_sub", "op_mul", "op_div", "op", "expr"]
        );

        let mut registry = ParserRegistry::default().with_extension(
            DynamicExtensionBuilder::default()
                .with(map_value::define_function)
                .with(map_value_error::define_function)
                .with(map_op_add::define_function)
                .with(map_op_sub::define_function)
                .with(map_op_mul::define_function)
                .with(map_op_div::define_function)
                .build(),
        );
        generator.install(&mut registry).unwrap();

        let (rest, result) = registry.parse("value", "42").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 42.0);

        let (rest, result) = registry.parse("op_add", "+ 40 2").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 42.0);

        let (rest, result) = registry.parse("op_sub", "- 40 2").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 38.0);

        let (rest, result) = registry.parse("op_mul", "* 40 2").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 80.0);

        let (rest, result) = registry.parse("op_div", "/ 40 2").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 20.0);

        let (rest, result) = registry.parse("op", "(+ 40 2)").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 42.0);

        let (rest, result) = registry.parse("expr", "(+ 40 2)").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 42.0);

        let (rest, result) = registry.parse("expr", "(+ (* 4 10) 2)").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 42.0);

        let (rest, result) = registry.parse("expr", "(+ (* 4 10) (/ 4 2))").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 42.0);

        let (rest, result) = registry
            .parse("expr", "(+ (* 4 10) (/ (- 5 1) 2))")
            .unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 42.0);
    }

    #[test]
    fn test_extending() {
        let grammar = std::fs::read_to_string("./resources/extending.txt").unwrap();
        let generator = Generator::new(&grammar).unwrap();
        assert_eq!(
            generator
                .parsers
                .iter()
                .map(|(k, _, _)| k.as_str())
                .collect::<Vec<_>>(),
            vec!["main", "main2", "main3"]
        );

        let mut registry = ParserRegistry::default();
        generator.install(&mut registry).unwrap();

        let (rest, result) = registry.parse("main", "bar").unwrap();
        assert_eq!(rest, "");
        assert!(result.is::<String>());
    }
}
