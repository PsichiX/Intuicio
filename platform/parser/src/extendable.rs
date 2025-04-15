use crate::{
    ParseResult, Parser, ParserExt, ParserHandle, ParserRegistry, alternation::AlternationParser,
};
use std::sync::RwLock;

pub mod shorthand {
    use super::*;

    pub fn ext_exchange(parser: ParserHandle) -> ParserHandle {
        ExtendableParser::exchange(parser).into_handle()
    }

    pub fn ext_depth(parser: ParserHandle) -> ParserHandle {
        ExtendableParser::depth(parser).into_handle()
    }

    pub fn ext_variants() -> ParserHandle {
        ExtendableParser::variants().into_handle()
    }

    pub fn ext_wrap(parser: ParserHandle, extendable: ParserHandle) -> ParserHandle {
        ExtendableWrapperParser::new(parser, extendable).into_handle()
    }
}

#[derive(Clone)]
enum ExtendableParserInner {
    Exchange(ParserHandle),
    Depth(ParserHandle),
    Variants(AlternationParser),
}

pub struct ExtendableParser {
    inner: RwLock<ExtendableParserInner>,
}

impl ExtendableParser {
    pub fn exchange(parser: ParserHandle) -> Self {
        Self {
            inner: RwLock::new(ExtendableParserInner::Exchange(parser)),
        }
    }

    pub fn depth(parser: ParserHandle) -> Self {
        Self {
            inner: RwLock::new(ExtendableParserInner::Depth(parser)),
        }
    }

    pub fn variants() -> Self {
        Self {
            inner: RwLock::new(ExtendableParserInner::Variants(Default::default())),
        }
    }
}

impl Parser for ExtendableParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        if let Ok(inner) = self.inner.read() {
            match &*inner {
                ExtendableParserInner::Exchange(parser) => parser.parse(registry, input),
                ExtendableParserInner::Depth(parser) => parser.parse(registry, input),
                ExtendableParserInner::Variants(parser) => parser.parse(registry, input),
            }
        } else {
            Err("ExtendableParser cannot be read".into())
        }
    }

    fn extend(&self, parser: ParserHandle) {
        if let Ok(mut inner) = self.inner.write() {
            match &mut *inner {
                ExtendableParserInner::Exchange(inner) => {
                    *inner = parser;
                }
                ExtendableParserInner::Depth(inner) => {
                    parser.extend(inner.clone());
                    *inner = parser;
                }
                ExtendableParserInner::Variants(inner) => {
                    inner.prepend(parser);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct ExtendableWrapperParser {
    parser: ParserHandle,
    extendable: ParserHandle,
}

impl ExtendableWrapperParser {
    pub fn new(parser: ParserHandle, extendable: ParserHandle) -> Self {
        Self { parser, extendable }
    }
}

impl Parser for ExtendableWrapperParser {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        self.parser.parse(registry, input)
    }

    fn extend(&self, parser: ParserHandle) {
        self.extendable.extend(parser);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ParserRegistry,
        extendable::{ExtendableParser, ExtendableWrapperParser},
        shorthand::{
            ext_depth, ext_exchange, ext_variants, ext_wrap, lit, oc, seq, slot_empty, ws,
        },
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_extendable() {
        is_async::<ExtendableParser>();
        is_async::<ExtendableWrapperParser>();

        let registry = ParserRegistry::default();
        let keyword_foo = lit("foo");
        let keyword_bar = lit("bar");
        let keyword_zee = lit("zee");

        let exchange = ext_exchange(keyword_foo.clone());
        assert!(exchange.parse(&registry, "foo").is_ok());
        assert!(exchange.parse(&registry, "bar").is_err());
        assert!(exchange.parse(&registry, "zee").is_err());
        exchange.extend(keyword_bar.clone());
        assert!(exchange.parse(&registry, "foo").is_err());
        assert!(exchange.parse(&registry, "bar").is_ok());
        assert!(exchange.parse(&registry, "zee").is_err());
        exchange.extend(keyword_zee.clone());
        assert!(exchange.parse(&registry, "foo").is_err());
        assert!(exchange.parse(&registry, "bar").is_err());
        assert!(exchange.parse(&registry, "zee").is_ok());

        let variants = ext_variants();
        assert!(variants.parse(&registry, "foo").is_err());
        assert!(variants.parse(&registry, "bar").is_err());
        assert!(variants.parse(&registry, "zee").is_err());
        variants.extend(keyword_foo);
        assert!(variants.parse(&registry, "foo").is_ok());
        assert!(variants.parse(&registry, "bar").is_err());
        assert!(variants.parse(&registry, "zee").is_err());
        variants.extend(keyword_bar);
        assert!(variants.parse(&registry, "foo").is_ok());
        assert!(variants.parse(&registry, "bar").is_ok());
        assert!(variants.parse(&registry, "zee").is_err());
        variants.extend(keyword_zee);
        assert!(variants.parse(&registry, "foo").is_ok());
        assert!(variants.parse(&registry, "bar").is_ok());
        assert!(variants.parse(&registry, "zee").is_ok());

        let signature = seq([lit("fn"), ws(), lit("foo"), lit("("), lit(")")]);
        let depth = ext_depth(signature);
        depth.parse(&registry, "fn foo()").unwrap();
        let async_signature = {
            let slot = slot_empty();
            ext_wrap(seq([lit("async"), ws(), slot.clone()]), slot)
        };
        async_signature.parse(&registry, "async ").unwrap();
        depth.extend(async_signature);
        depth.parse(&registry, "async fn foo()").unwrap();

        let signature = {
            let slot = slot_empty();
            ext_wrap(oc(slot.clone(), lit("("), lit(")")), slot)
        };
        assert!(signature.parse(&registry, "(foo)").is_err());
        signature.extend(lit("foo"));
        assert!(
            signature
                .parse(&registry, "(foo)")
                .unwrap()
                .1
                .is::<String>()
        );
    }
}
