use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserOutput, ParserRegistry};
use std::{error::Error, sync::Arc};

pub mod shorthand {
    use super::*;

    pub fn pratt(tokenizer_parser: ParserHandle, rules: Vec<Vec<PrattParserRule>>) -> ParserHandle {
        let mut result = PrattParser::new(tokenizer_parser);
        for rule in rules {
            result.push_rules(rule);
        }
        result.into_handle()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrattParserAssociativity {
    #[default]
    Left,
    Right,
}

#[derive(Clone)]
pub enum PrattParserRule {
    Prefix {
        operator: Arc<dyn Fn(&ParserOutput) -> bool + Send + Sync>,
        transformer: Arc<dyn Fn(ParserOutput) -> ParserOutput + Send + Sync>,
    },
    Postfix {
        operator: Arc<dyn Fn(&ParserOutput) -> bool + Send + Sync>,
        transformer: Arc<dyn Fn(ParserOutput) -> ParserOutput + Send + Sync>,
    },
    Infix {
        operator: Arc<dyn Fn(&ParserOutput) -> bool + Send + Sync>,
        transformer: Arc<dyn Fn(ParserOutput, ParserOutput) -> ParserOutput + Send + Sync>,
        associativity: PrattParserAssociativity,
    },
}

impl PrattParserRule {
    pub fn prefx_raw(
        operator: impl Fn(&ParserOutput) -> bool + Send + Sync + 'static,
        transformer: impl Fn(ParserOutput) -> ParserOutput + Send + Sync + 'static,
    ) -> Self {
        Self::Prefix {
            operator: Arc::new(operator),
            transformer: Arc::new(transformer),
        }
    }

    pub fn prefix<O: PartialEq + Send + Sync + 'static, V: Send + Sync + 'static>(
        operator: O,
        transformer: impl Fn(V) -> V + Send + Sync + 'static,
    ) -> Self {
        Self::prefx_raw(
            move |token| {
                token
                    .read::<O>()
                    .map(|op| *op == operator)
                    .unwrap_or_default()
            },
            move |value| {
                let value = value.consume::<V>().ok().unwrap();
                let result = (transformer)(value);
                ParserOutput::new(result).ok().unwrap()
            },
        )
    }

    pub fn postfix_raw(
        operator: impl Fn(&ParserOutput) -> bool + Send + Sync + 'static,
        transformer: impl Fn(ParserOutput) -> ParserOutput + Send + Sync + 'static,
    ) -> Self {
        Self::Postfix {
            operator: Arc::new(operator),
            transformer: Arc::new(transformer),
        }
    }

    pub fn postfix<O: PartialEq + Send + Sync + 'static, V: Send + Sync + 'static>(
        operator: O,
        transformer: impl Fn(V) -> V + Send + Sync + 'static,
    ) -> Self {
        Self::postfix_raw(
            move |token| {
                token
                    .read::<O>()
                    .map(|op| *op == operator)
                    .unwrap_or_default()
            },
            move |value| {
                let value = value.consume::<V>().ok().unwrap();
                let result = (transformer)(value);
                ParserOutput::new(result).ok().unwrap()
            },
        )
    }

    pub fn infix_raw(
        operator: impl Fn(&ParserOutput) -> bool + Send + Sync + 'static,
        transformer: impl Fn(ParserOutput, ParserOutput) -> ParserOutput + Send + Sync + 'static,
        associativity: PrattParserAssociativity,
    ) -> Self {
        Self::Infix {
            operator: Arc::new(operator),
            transformer: Arc::new(transformer),
            associativity,
        }
    }

    pub fn infix<O: PartialEq + Send + Sync + 'static, V: Send + Sync + 'static>(
        operator: O,
        transformer: impl Fn(V, V) -> V + Send + Sync + 'static,
        associativity: PrattParserAssociativity,
    ) -> Self {
        Self::infix_raw(
            move |token| {
                token
                    .read::<O>()
                    .map(|op| *op == operator)
                    .unwrap_or_default()
            },
            move |lhs, rhs| {
                let lhs = lhs.consume::<V>().ok().unwrap();
                let rhs = rhs.consume::<V>().ok().unwrap();
                let result = (transformer)(lhs, rhs);
                ParserOutput::new(result).ok().unwrap()
            },
            associativity,
        )
    }

    fn flip_binding_power(&self) -> bool {
        matches!(
            self,
            Self::Infix {
                associativity: PrattParserAssociativity::Right,
                ..
            }
        )
    }
}

#[derive(Clone)]
pub struct PrattParser {
    tokenizer_parser: ParserHandle,
    /// [(rule, left binding power, right binding power)]
    rules: Vec<(PrattParserRule, usize, usize)>,
    binding_power_generator: usize,
}

impl PrattParser {
    pub fn new(tokenizer_parser: ParserHandle) -> Self {
        Self {
            tokenizer_parser,
            rules: vec![],
            binding_power_generator: 0,
        }
    }

    pub fn with_rules(mut self, rules: impl IntoIterator<Item = PrattParserRule>) -> Self {
        self.push_rules(rules);
        self
    }

    pub fn push_rules(&mut self, rules: impl IntoIterator<Item = PrattParserRule>) {
        let low = self.binding_power_generator + 1;
        let high = self.binding_power_generator + 2;
        self.binding_power_generator += 2;
        for rule in rules {
            if rule.flip_binding_power() {
                self.rules.push((rule, high, low));
            } else {
                self.rules.push((rule, low, high));
            }
        }
    }

    fn parse_inner(
        &self,
        tokens: &mut Vec<ParserOutput>,
        min_bp: usize,
    ) -> Result<ParserOutput, Box<dyn Error>> {
        let Some(mut lhs) = tokens.pop() else {
            return Err("Expected LHS token value".into());
        };
        if let Some((rule, _, rbp)) = self.find_prefix_rule(&lhs) {
            let rhs = self.parse_inner(tokens, rbp)?;
            if let PrattParserRule::Prefix { transformer, .. } = rule {
                lhs = (*transformer)(rhs);
            } else {
                return Err("Expected prefix rule".into());
            }
        }
        while let Some(op) = tokens.pop() {
            if let Some((rule, lbp, _)) = self.find_postfix_rule(&op) {
                if lbp < min_bp {
                    tokens.push(op);
                    break;
                }
                if let PrattParserRule::Postfix { transformer, .. } = rule {
                    lhs = (*transformer)(lhs);
                } else {
                    return Err("Expected postfix rule".into());
                }
                continue;
            }
            if let Some((rule, lbp, rbp)) = self.find_infix_rule(&op) {
                if lbp < min_bp {
                    tokens.push(op);
                    break;
                }
                let rhs = self.parse_inner(tokens, rbp)?;
                if let PrattParserRule::Infix { transformer, .. } = rule {
                    lhs = (*transformer)(lhs, rhs);
                } else {
                    return Err("Expected infix rule".into());
                }
                continue;
            }
            tokens.push(op);
            break;
        }
        Ok(lhs)
    }

    /// (rule, _, right binding power)
    fn find_prefix_rule(&self, token: &ParserOutput) -> Option<(&PrattParserRule, (), usize)> {
        self.rules
            .iter()
            .find(|(rule, _, _)| match rule {
                PrattParserRule::Prefix { operator, .. } => (*operator)(token),
                _ => false,
            })
            .map(|(rule, _, rbp)| (rule, (), *rbp))
    }

    /// (rule, left binding power, _)
    fn find_postfix_rule(&self, token: &ParserOutput) -> Option<(&PrattParserRule, usize, ())> {
        self.rules
            .iter()
            .find(|(rule, _, _)| match rule {
                PrattParserRule::Postfix { operator, .. } => (*operator)(token),
                _ => false,
            })
            .map(|(rule, lbp, _)| (rule, *lbp, ()))
    }

    /// (rule, left binding power, right binding power)
    fn find_infix_rule(&self, token: &ParserOutput) -> Option<(&PrattParserRule, usize, usize)> {
        self.rules
            .iter()
            .find(|(rule, _, _)| match rule {
                PrattParserRule::Infix { operator, .. } => (*operator)(token),
                _ => false,
            })
            .map(|(rule, lbp, rbp)| (rule, *lbp, *rbp))
    }
}

impl Parser for PrattParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        let (input, result) = self.tokenizer_parser.parse(registry, input)?;
        let mut tokens = match result.consume::<Vec<ParserOutput>>() {
            Ok(tokens) => tokens,
            Err(_) => {
                return Err("PrattParser expects `Vec<ParserOutput>` tokenization result".into());
            }
        };
        tokens.reverse();
        let result = self.parse_inner(&mut tokens, 0)?;
        if !tokens.is_empty() {
            return Err("PrattParser did not consumed all tokens".into());
        }
        Ok((input, result))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserHandle, ParserRegistry,
        pratt::{PrattParser, PrattParserAssociativity, PrattParserRule},
        shorthand::{
            alt, inject, list, lit, map, map_err, number_float, oc, ows, pratt, prefix, suffix,
        },
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Operator {
        Add,
        Sub,
        Mul,
        Div,
        // takes integer part.
        Hash,
        // takes fractional part.
        Bang,
    }

    impl std::fmt::Display for Operator {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Add => write!(f, "+"),
                Self::Sub => write!(f, "-"),
                Self::Mul => write!(f, "*"),
                Self::Div => write!(f, "/"),
                Self::Hash => write!(f, "#"),
                Self::Bang => write!(f, "!"),
            }
        }
    }

    #[derive(Debug)]
    enum Expression {
        Number(f32),
        UnaryOperation {
            op: Operator,
            value: Box<Expression>,
        },
        BinaryOperation {
            op: Operator,
            lhs: Box<Expression>,
            rhs: Box<Expression>,
        },
    }

    impl Expression {
        fn eval(&self) -> f32 {
            match self {
                Self::Number(value) => *value,
                Self::UnaryOperation { op, value } => match op {
                    Operator::Hash => value.eval().floor(),
                    Operator::Bang => value.eval().fract(),
                    _ => unreachable!(),
                },
                Self::BinaryOperation { op, lhs, rhs } => match op {
                    Operator::Add => lhs.eval() + rhs.eval(),
                    Operator::Sub => lhs.eval() - rhs.eval(),
                    Operator::Mul => lhs.eval() * rhs.eval(),
                    Operator::Div => lhs.eval() / rhs.eval(),
                    _ => unreachable!(),
                },
            }
        }
    }

    impl std::fmt::Display for Expression {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Number(value) => write!(f, "{value}"),
                Self::UnaryOperation { value, op } => write!(f, "({op} {value})"),
                Self::BinaryOperation { op, lhs, rhs } => write!(f, "({op} {lhs} {rhs})"),
            }
        }
    }

    fn number() -> ParserHandle {
        map_err(
            map(number_float(), |value: String| {
                Expression::Number(value.parse().unwrap())
            }),
            |_| "Expected number".into(),
        )
    }

    fn op() -> ParserHandle {
        map_err(
            map(
                alt([lit("+"), lit("-"), lit("*"), lit("/"), lit("#"), lit("!")]),
                |value: String| match value.as_str() {
                    "+" => Operator::Add,
                    "-" => Operator::Sub,
                    "*" => Operator::Mul,
                    "/" => Operator::Div,
                    "#" => Operator::Hash,
                    "!" => Operator::Bang,
                    _ => unreachable!(),
                },
            ),
            |_| "Expected operator".into(),
        )
    }

    fn sub_expr() -> ParserHandle {
        map_err(
            oc(
                inject("expr"),
                suffix(lit("("), ows()),
                prefix(lit(")"), ows()),
            ),
            |_| "Expected sub-expression".into(),
        )
    }

    fn item() -> ParserHandle {
        alt([inject("number"), inject("op"), inject("sub_expr")])
    }

    fn expr_tokenizer() -> ParserHandle {
        list(inject("item"), ows(), true)
    }

    fn expr() -> ParserHandle {
        pratt(
            inject("expr_tokenizer"),
            vec![
                vec![
                    PrattParserRule::infix(
                        Operator::Add,
                        |lhs, rhs| Expression::BinaryOperation {
                            op: Operator::Add,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        },
                        PrattParserAssociativity::Left,
                    ),
                    PrattParserRule::infix(
                        Operator::Sub,
                        |lhs, rhs| Expression::BinaryOperation {
                            op: Operator::Sub,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        },
                        PrattParserAssociativity::Left,
                    ),
                ],
                vec![
                    PrattParserRule::infix(
                        Operator::Mul,
                        |lhs, rhs| Expression::BinaryOperation {
                            op: Operator::Mul,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        },
                        PrattParserAssociativity::Left,
                    ),
                    PrattParserRule::infix(
                        Operator::Div,
                        |lhs, rhs| Expression::BinaryOperation {
                            op: Operator::Div,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        },
                        PrattParserAssociativity::Left,
                    ),
                ],
                vec![PrattParserRule::prefix(Operator::Hash, |value| {
                    Expression::UnaryOperation {
                        op: Operator::Hash,
                        value: Box::new(value),
                    }
                })],
                vec![PrattParserRule::postfix(Operator::Bang, |value| {
                    Expression::UnaryOperation {
                        op: Operator::Bang,
                        value: Box::new(value),
                    }
                })],
            ],
        )
    }

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_pratt() {
        is_async::<PrattParser>();

        let registry = ParserRegistry::default()
            .with_parser("number", number())
            .with_parser("op", op())
            .with_parser("sub_expr", sub_expr())
            .with_parser("item", item())
            .with_parser("expr_tokenizer", expr_tokenizer())
            .with_parser("expr", expr());
        let (rest, result) = registry.parse("expr", "(((0)))").unwrap();
        assert_eq!(rest, "");
        let result = result.consume::<Expression>().ok().unwrap();
        assert_eq!(result.to_string(), "0");
        assert_eq!(result.eval(), 0.0);
        let (rest, result) = registry.parse("expr", "(3 + 4) * 2 - 1 / 5").unwrap();
        assert_eq!(rest, "");
        let result = result.consume::<Expression>().ok().unwrap();
        assert_eq!(result.to_string(), "(- (* (+ 3 4) 2) (/ 1 5))");
        assert_eq!(result.eval(), 13.8);
        let (rest, result) = registry.parse("expr", "#1.2 + 3.4!").unwrap();
        assert_eq!(rest, "");
        let result = result.consume::<Expression>().ok().unwrap();
        assert_eq!(result.to_string(), "(+ (# 1.2) (! 3.4))");
        assert_eq!(result.eval(), 1.4000001);
        let (rest, result) = registry.parse("expr", "#(1.2 - 3.4)!").unwrap();
        assert_eq!(rest, "");
        let result = result.consume::<Expression>().ok().unwrap();
        assert_eq!(result.to_string(), "(# (! (- 1.2 3.4)))");
        assert_eq!(result.eval(), -1.0);
    }
}
