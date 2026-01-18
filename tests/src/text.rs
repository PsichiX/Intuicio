use intuicio_core::{
    IntuicioStruct, context::Context, registry::Registry, types::struct_type::NativeStructBuilder,
};
use intuicio_data::managed::gc::ManagedGc;
use intuicio_derive::*;
use intuicio_framework_text::{Name, Text, name, text};
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
    pub fn get(this: ManagedGc<Self>, key: Name) -> Text {
        this.try_read()
            .unwrap()
            .map
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }

    #[intuicio_method()]
    pub fn set(mut this: ManagedGc<Self>, key: Name, value: Text) {
        this.try_write().unwrap().map.insert(key, value);
    }
}

#[test]
fn test_text() {
    let mut context = Context::new(10240, 10240);
    let mut registry = Registry::default().with_basic_types();

    registry.add_type(Name::define_struct(&registry));
    registry.add_type(Text::define_struct(&registry));
    registry.add_type(NativeStructBuilder::new::<ManagedGc<Loc>>().build());
    registry.add_type(Loc::define_struct(&registry));
    let get = registry.add_function(Loc::get__define_function(&registry));
    let set = registry.add_function(Loc::set__define_function(&registry));

    let foo_name = name!("foo");
    let bar_name = name!("bar");
    let foo_text = text!("Foo");
    let bar_text = text!("Bar");

    let loc = ManagedGc::<Loc>::default();
    Loc::set(loc.reference(), foo_name, foo_text.clone());

    let (v,) = get.call::<(Text,), _>(&mut context, &registry, (loc.reference(), foo_name), false);
    assert_eq!(v, foo_text);

    set.call::<(), _>(
        &mut context,
        &registry,
        (loc.reference(), bar_name, bar_text.clone()),
        false,
    );
    assert_eq!(Loc::get(loc, bar_name), bar_text);
}
