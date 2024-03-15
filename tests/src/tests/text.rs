use intuicio_core::prelude::*;
use intuicio_data::prelude::*;
use intuicio_derive::*;
use intuicio_framework_text::{Name, Text};
use std::collections::HashMap;

#[derive(IntuicioStruct, Debug, Default)]
#[intuicio(module_name = "test")]
struct Loc {
    #[intuicio(ignore)]
    map: HashMap<Name, Text>,
}

#[intuicio_methods(module_name = "test")]
impl Loc {
    #[intuicio_method()]
    pub fn get(this: ManagedBox<Self>, key: Name) -> Text {
        this.read()
            .unwrap()
            .map
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }

    #[intuicio_method()]
    pub fn set(mut this: ManagedBox<Self>, key: Name, value: Text) {
        this.write().unwrap().map.insert(key, value);
    }
}

#[test]
fn test_text() {
    let mut context = Context::new(10240, 10240);
    let mut registry = Registry::default().with_basic_types();

    registry.add_type(Name::define_struct(&registry));
    registry.add_type(Text::define_struct(&registry));
    registry.add_type(NativeStructBuilder::new::<ManagedBox<Loc>>().build());
    registry.add_type(Loc::define_struct(&registry));
    let get = registry.add_function(Loc::get__define_function(&registry));
    let set = registry.add_function(Loc::set__define_function(&registry));

    let foo_name = Name::new_static("foo");
    let bar_name = Name::new_static("bar");
    let foo_text = Text::new("Foo");
    let bar_text = Text::new("Bar");

    let loc = ManagedBox::<Loc>::default();
    Loc::set(loc.clone(), foo_name, foo_text.clone());

    let (v,) = get.call::<(Text,), _>(&mut context, &registry, (loc.clone(), foo_name), false);
    assert_eq!(v, foo_text);

    set.call::<(), _>(
        &mut context,
        &registry,
        (loc.clone(), bar_name, bar_text.clone()),
        false,
    );
    assert_eq!(Loc::get(loc, bar_name), bar_text);
}
