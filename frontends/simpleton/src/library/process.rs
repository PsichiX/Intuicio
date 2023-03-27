use crate::{Array, Integer, Map, Reference, Text};
use intuicio_core::{registry::Registry, IntuicioStruct};
use intuicio_derive::{intuicio_function, IntuicioStruct};
use std::process::Command;

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "CommandOutput", module_name = "process")]
pub struct CommandOutput {
    pub status: Reference,
    pub stdout: Reference,
    pub stderr: Reference,
}

#[intuicio_function(module_name = "process")]
pub fn panic(message: Reference) -> Reference {
    panic!("{}", message.read::<Text>().unwrap().as_str());
}

#[intuicio_function(module_name = "process", use_registry)]
pub fn command(
    registry: &Registry,
    program: Reference,
    args: Reference,
    envs: Reference,
) -> Reference {
    let program = program.read::<Text>().unwrap();
    let output = Command::new(program.as_str())
        .args(
            args.read::<Array>()
                .unwrap()
                .iter()
                .map(|arg| arg.read::<Text>().unwrap().as_str().to_owned()),
        )
        .envs(envs.read::<Map>().unwrap().iter().map(|(key, value)| {
            (
                key.to_owned(),
                value.read::<Text>().unwrap().as_str().to_owned(),
            )
        }))
        .spawn()
        .unwrap_or_else(|_| panic!("Could not run program: `{}`", program.as_str()))
        .wait_with_output()
        .unwrap_or_else(|_| panic!("Failed to wait for program: `{}`", program.as_str()));
    let output = CommandOutput {
        status: Reference::new_integer(output.status.code().unwrap_or(0) as Integer, registry),
        stdout: Reference::new_text(
            String::from_utf8_lossy(&output.stdout).to_string(),
            registry,
        ),
        stderr: Reference::new_text(
            String::from_utf8_lossy(&output.stderr).to_string(),
            registry,
        ),
    };
    Reference::new(output, registry)
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(CommandOutput::define_struct(registry));
    registry.add_function(panic::define_function(registry));
    registry.add_function(command::define_function(registry));
}
