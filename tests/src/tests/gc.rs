use gc::{Finalize as GcFinalize, Gc, Trace as GcTrace};
use intuicio_core::prelude::*;
use intuicio_derive::*;

#[intuicio_function(module_name = "test")]
fn add(a: Gc<i32>, b: Gc<i32>) -> Gc<i32> {
    let c = *a + *b;
    println!("add | {} + {} = {}", *a, *b, c);
    Gc::new(c)
}

#[derive(IntuicioStruct, Debug, Default, GcFinalize, GcTrace)]
#[intuicio(module_name = "test")]
struct Adder {
    a: Gc<i32>,
    b: Gc<i32>,
}

#[intuicio_methods(module_name = "test")]
impl Adder {
    #[intuicio_method()]
    pub fn calculate(self) -> Gc<i32> {
        let c = *self.a + *self.b;
        println!("Adder::calculate | {} + {} = {}", *self.a, *self.b, c);
        Gc::new(c)
    }
}

#[test]
fn test_gc() {
    let mut context = Context::new(1024, 1024, 1024);
    let mut registry = Registry::default().with_basic_types();
    registry.add_struct(NativeStructBuilder::new::<Gc<i32>>().build());
    let add = registry.add_function(add::define_function(&registry));
    registry.add_struct(Adder::define_struct(&registry));
    let calculate = registry.add_function(Adder::calculate__define_function(&registry));

    let x = Gc::new(40);
    let y = Gc::new(2);

    let (c,) = add.call::<(Gc<i32>,), _>(&mut context, &registry, (x.clone(), y.clone()), true);
    assert_eq!(*c, 42);

    context.stack().push(x.clone());
    context.stack().push(y.clone());
    add::intuicio_function(&mut context, &registry);
    assert_eq!(*context.stack().pop::<Gc<i32>>().unwrap(), 42);

    let (c,) = calculate.call::<(Gc<i32>,), _>(
        &mut context,
        &registry,
        (Adder {
            a: x.clone(),
            b: y.clone(),
        },),
        true,
    );
    assert_eq!(*c, 42);

    context.stack().push(Adder { a: y, b: x });
    Adder::calculate__intuicio_function(&mut context, &registry);
    assert_eq!(*context.stack().pop::<Gc<i32>>().unwrap(), 42);
}
