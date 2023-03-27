use crate::{Array, Boolean, Integer, Reference, Text};
use intuicio_core::{registry::Registry, IntuicioStruct};
use intuicio_derive::{intuicio_method, intuicio_methods, IntuicioStruct};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Server", module_name = "net_server")]
pub struct Server {
    #[intuicio(ignore)]
    listener: Option<TcpListener>,
}

#[intuicio_methods(module_name = "net_server")]
impl Server {
    #[intuicio_method(use_registry)]
    pub fn new(registry: &Registry, address: Reference) -> Reference {
        let address = address.read::<Text>().unwrap();
        Reference::new(
            Server {
                listener: Some(TcpListener::bind(address.as_str()).unwrap()),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn set_nonblocking(registry: &Registry, server: Reference, mode: Reference) -> Reference {
        let server = server.read::<Server>().unwrap();
        let mode = *mode.read::<Boolean>().unwrap();
        Reference::new_boolean(
            server
                .listener
                .as_ref()
                .unwrap()
                .set_nonblocking(mode)
                .is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn accept(registry: &Registry, server: Reference) -> Reference {
        let server = server.read::<Server>().unwrap();
        if let Ok((stream, _)) = server.listener.as_ref().unwrap().accept() {
            Reference::new(
                Channel {
                    stream: Some(stream),
                },
                registry,
            )
        } else {
            Reference::null()
        }
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Channel", module_name = "net_channel")]
pub struct Channel {
    #[intuicio(ignore)]
    stream: Option<TcpStream>,
}

#[intuicio_methods(module_name = "net_channel")]
impl Channel {
    #[intuicio_method(use_registry)]
    pub fn connect(registry: &Registry, address: Reference) -> Reference {
        let address = address.read::<Text>().unwrap();
        let stream = TcpStream::connect(address.as_str()).unwrap();
        Reference::new(
            Channel {
                stream: Some(stream),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn set_nonblocking(registry: &Registry, channel: Reference, mode: Reference) -> Reference {
        let channel = channel.read::<Channel>().unwrap();
        let mode = *mode.read::<Boolean>().unwrap();
        Reference::new_boolean(
            channel
                .stream
                .as_ref()
                .unwrap()
                .set_nonblocking(mode)
                .is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn set_no_delay(registry: &Registry, channel: Reference, mode: Reference) -> Reference {
        let channel = channel.read::<Channel>().unwrap();
        let mode = *mode.read::<Boolean>().unwrap();
        Reference::new_boolean(
            channel.stream.as_ref().unwrap().set_nodelay(mode).is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn read(registry: &Registry, mut channel: Reference, size: Reference) -> Reference {
        let mut channel = channel.write::<Channel>().unwrap();
        let size = *size.read::<Integer>().unwrap() as usize;
        let mut buffer = vec![0; size];
        if channel.stream.as_mut().unwrap().read(&mut buffer).is_ok() {
            Reference::new_array(
                buffer
                    .into_iter()
                    .map(|byte| Reference::new_integer(byte as Integer, registry))
                    .collect(),
                registry,
            )
        } else {
            Reference::null()
        }
    }

    #[intuicio_method(use_registry)]
    pub fn write(registry: &Registry, mut channel: Reference, buffer: Reference) -> Reference {
        let mut channel = channel.write::<Channel>().unwrap();
        let buffer = buffer
            .read::<Array>()
            .unwrap()
            .iter()
            .map(|byte| *byte.read::<Integer>().unwrap() as u8)
            .collect::<Vec<u8>>();
        Reference::new_boolean(
            channel.stream.as_mut().unwrap().write(&buffer).is_ok(),
            registry,
        )
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Server::define_struct(registry));
    registry.add_function(Server::new__define_function(registry));
    registry.add_function(Server::set_nonblocking__define_function(registry));
    registry.add_function(Server::accept__define_function(registry));
    registry.add_struct(Channel::define_struct(registry));
    registry.add_function(Channel::connect__define_function(registry));
    registry.add_function(Channel::set_nonblocking__define_function(registry));
    registry.add_function(Channel::set_no_delay__define_function(registry));
    registry.add_function(Channel::read__define_function(registry));
    registry.add_function(Channel::write__define_function(registry));
}
