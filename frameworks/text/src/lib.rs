use intuicio_derive::IntuicioStruct;
use std::{cell::RefCell, marker::PhantomData, ops::Deref, ptr::NonNull, str::FromStr, sync::Arc};
use string_interner::{
    backend::{Backend, BufferBackend},
    StringInterner,
};

thread_local! {
    static INTERNER: RefCell<StringInterner<BufferBackend>> = RefCell::new(StringInterner::<BufferBackend>::new());
}

#[derive(IntuicioStruct, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name {
    #[intuicio(ignore)]
    symbol: <BufferBackend as Backend>::Symbol,
    #[intuicio(ignore)]
    _phantom: PhantomData<NonNull<()>>,
}

impl Default for Name {
    fn default() -> Self {
        Self::new_static("")
    }
}

impl Name {
    pub fn new(value: impl AsRef<str>) -> Self {
        INTERNER.with_borrow_mut(|interner| Self {
            symbol: interner.get_or_intern(value),
            _phantom: PhantomData,
        })
    }

    pub fn new_static(value: &'static str) -> Self {
        INTERNER.with_borrow_mut(|interner| Self {
            symbol: interner.get_or_intern_static(value),
            _phantom: PhantomData,
        })
    }

    pub fn symbol(this: &Self) -> <BufferBackend as Backend>::Symbol {
        this.symbol
    }

    pub fn read(this: &Self) -> &str {
        INTERNER.with_borrow(|interner| {
            interner
                .resolve(this.symbol)
                .map(|content| unsafe { std::mem::transmute(content) })
                .unwrap_or_else(|| {
                    panic!("Could not resolve TextId with symbol: {:?}", this.symbol)
                })
        })
    }
}

impl Deref for Name {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        Self::read(self)
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        Self::read(self)
    }
}

impl FromStr for Name {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

impl From<&str> for Name {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Debug for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", Self::read(self))
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Self::read(self))
    }
}

#[derive(IntuicioStruct, Clone, PartialEq, PartialOrd, Hash)]
pub struct Text {
    #[intuicio(ignore)]
    content: Arc<str>,
}

impl Default for Text {
    fn default() -> Self {
        Self {
            content: Arc::from(""),
        }
    }
}

impl Text {
    pub fn new(value: impl AsRef<str>) -> Self {
        Self {
            content: Arc::from(value.as_ref()),
        }
    }

    pub fn into_inner(self) -> Arc<str> {
        self.content
    }

    pub fn read(this: &Self) -> &str {
        &this.content
    }

    pub fn ptr_eq(a: &Self, b: &Self) -> bool {
        Arc::ptr_eq(&a.content, &b.content)
    }
}

impl Deref for Text {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        Self::read(self)
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        Self::read(self)
    }
}

impl FromStr for Text {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", Self::read(self))
    }
}

impl std::fmt::Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Self::read(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_name() {
        let a = Name::new_static("foo");
        let b = Name::new_static("foo");
        let c = a;
        let d = Name::new_static("bar");

        assert_eq!(a.as_ref(), "foo");
        assert_eq!(b.as_ref(), "foo");
        assert_eq!(c.as_ref(), "foo");
        assert_eq!(d.as_ref(), "bar");
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(b, c);
        assert_ne!(a, d);
        assert_ne!(b, d);
        assert_ne!(c, d);
        assert_eq!(Name::symbol(&a), Name::symbol(&b));
        assert_eq!(Name::symbol(&a), Name::symbol(&c));
        assert_eq!(Name::symbol(&b), Name::symbol(&c));
        assert_ne!(Name::symbol(&a), Name::symbol(&d));
        assert_ne!(Name::symbol(&b), Name::symbol(&d));
        assert_ne!(Name::symbol(&c), Name::symbol(&d));
    }

    #[test]
    fn test_text() {
        let a = Text::new("foo");
        let b = Text::new("foo");
        let c = a.clone();
        let d = Text::new("bar");

        assert_eq!(a.as_ref(), "foo");
        assert_eq!(b.as_ref(), "foo");
        assert_eq!(c.as_ref(), "foo");
        assert_eq!(d.as_ref(), "bar");
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(b, c);
        assert_ne!(a, d);
        assert_ne!(b, d);
        assert_ne!(c, d);
        assert!(!Text::ptr_eq(&a, &b));
        assert!(Text::ptr_eq(&a, &c));
        assert!(!Text::ptr_eq(&b, &c));
        assert!(!Text::ptr_eq(&a, &d));
        assert!(!Text::ptr_eq(&b, &d));
        assert!(!Text::ptr_eq(&c, &d));
    }

    #[test]
    fn test_name_text_map() {
        let mut map = HashMap::new();
        map.insert(Name::new("foo"), Text::new("Foo"));
        let bar = Text::new("Bar");
        map.insert(Name::new("bar"), bar.clone());
        map.insert(Name::new("bar2"), bar);

        assert_eq!(map.len(), 3);
        assert_eq!(map.get(&Name::new("foo")).unwrap().as_ref(), "Foo");
        assert_eq!(map.get(&Name::new("bar")).unwrap().as_ref(), "Bar");
        assert_eq!(map.get(&Name::new("bar2")).unwrap().as_ref(), "Bar");
    }
}
