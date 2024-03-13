use intuicio_core::prelude::*;
use intuicio_derive::*;

#[intuicio_function(module_name = "test")]
fn add(a: i32, b: i32) -> i32 {
    let c = a + b;
    println!("add | {} + {} = {}", a, b, c);
    c
}

#[derive(IntuicioStruct, Debug, Default)]
#[intuicio(module_name = "test")]
struct Adder {
    a: i32,
    b: i32,
}

#[intuicio_methods(module_name = "test")]
impl Adder {
    #[intuicio_method()]
    pub fn calculate(self) -> i32 {
        let c = self.a + self.b;
        println!("Adder::calculate | {} + {} = {}", self.a, self.b, c);
        c
    }
}

#[test]
fn test_derive() {
    let mut context = Context::new(10240, 10240);
    let mut registry = Registry::default().with_basic_types();
    let add = registry.add_function(add::define_function(&registry));
    registry.add_type(Adder::define_struct(&registry));
    let calculate = registry.add_function(Adder::calculate__define_function(&registry));

    let (c,) = add.call::<(i32,), _>(&mut context, &registry, (40_i32, 2_i32), true);
    assert_eq!(c, 42);

    context.stack().push(2_i32);
    context.stack().push(40_i32);
    add::intuicio_function(&mut context, &registry);
    assert_eq!(context.stack().pop::<i32>().unwrap(), 42);

    let (c,) = calculate.call::<(i32,), _>(&mut context, &registry, (Adder { a: 40, b: 2 },), true);
    assert_eq!(c, 42);

    context.stack().push(Adder { a: 40, b: 2 });
    Adder::calculate__intuicio_function(&mut context, &registry);
    assert_eq!(context.stack().pop::<i32>().unwrap(), 42);
}
