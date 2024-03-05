mod config;
mod library;
mod scripting;

use crate::scripting::Scripting;
use config::Config;
use std::path::Path;
use tetra::{
    graphics::{self, Color},
    time::Timestep,
    Context, ContextBuilder, State,
};

struct GameState {
    scripting: Scripting,
}

impl State for GameState {
    fn update(&mut self, ctx: &mut Context) -> tetra::Result {
        self.scripting.update(ctx);
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> tetra::Result {
        graphics::clear(ctx, Color::rgb(0.1, 0.1, 0.1));
        self.scripting.draw(ctx);
        Ok(())
    }
}

fn main() -> tetra::Result {
    let config = Path::new("./resources/config.toml");
    let config = if config.exists() {
        toml::from_str::<Config>(&std::fs::read_to_string(config).unwrap()).unwrap()
    } else {
        Default::default()
    };
    ContextBuilder::new(
        &config.game.title,
        config.game.window_width as _,
        config.game.window_height as _,
    )
    .fullscreen(config.game.fullscreen)
    .show_mouse(true)
    .quit_on_escape(true)
    .timestep(Timestep::Fixed(30.0))
    .build()?
    .run(|ctx| {
        let mut scripting = Scripting::new(
            &config.assets,
            config.scripting.stack_capacity,
            config.scripting.registers_capacity,
            &config.scripting.entry,
            ctx,
        );
        scripting.initialize();
        Ok(GameState { scripting })
    })
}
