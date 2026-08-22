//! Building a parser from state kept in the registry.
use crate::{ParseResult, Parser, ParserExt, ParserHandle, ParserRegistry};
use std::sync::Arc;

/// Short constructors for this module.
pub mod shorthand {
    use super::*;

    /// See [`ExtensionParser`].
    pub fn ext<T: Send + Sync + 'static>(
        f: impl Fn(Arc<T>) -> ParserHandle + Send + Sync + 'static,
    ) -> ParserHandle {
        ExtensionParser::new(f).into_handle()
    }
}

/// Asks the [`ParserRegistry`] for the extension of type `T` and builds the
/// real parser from it, per parse.
///
/// This is how a grammar reaches outside itself - to a symbol table, a
/// counter, or a script host as [`dynamic`](crate::dynamic) does.
///
/// Fails when no extension of that type was added to the registry.
#[derive(Clone)]
pub struct ExtensionParser<T: Send + Sync + 'static> {
    parser_generator: Arc<dyn Fn(Arc<T>) -> ParserHandle + Send + Sync>,
}

impl<T: Send + Sync + 'static> ExtensionParser<T> {
    /// Builds the parser with `f` each time this one runs.
    pub fn new(f: impl Fn(Arc<T>) -> ParserHandle + Send + Sync + 'static) -> Self {
        Self {
            parser_generator: Arc::new(f),
        }
    }
}

impl<T: Send + Sync + 'static> Parser for ExtensionParser<T> {
    fn parse<'a>(&self, registry: &ParserRegistry, input: &'a str) -> ParseResult<'a> {
        if let Some(extension) = registry.extension::<T>() {
            (self.parser_generator)(extension).parse(registry, input)
        } else {
            Err("Could not get ExtensionParser extension!".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::RwLock;

    use crate::{
        ParserRegistry,
        extension::ExtensionParser,
        shorthand::{ext, lit},
    };

    fn is_async<T: Send + Sync>() {}

    #[derive(Default)]
    struct Extension {
        pub counter: RwLock<usize>,
    }

    #[test]
    fn test_extension() {
        is_async::<ExtensionParser<()>>();

        let registry = ParserRegistry::default().with_extension(Extension::default());
        let parser = ext::<Extension>(|extension| {
            *extension.counter.write().unwrap() += 1;
            lit("foo")
        });
        let (rest, _) = parser.parse(&registry, "foo").unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            *registry
                .extension::<Extension>()
                .unwrap()
                .counter
                .read()
                .unwrap(),
            1
        );
    }
}
