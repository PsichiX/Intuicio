use intuicio_core::{IntuicioStruct, IntuicioVersion, core_version, registry::Registry};
use intuicio_derive::*;
use intuicio_frontend_simpleton::prelude::{bytes::Bytes, *};
use reqwest::blocking::Client;
use std::collections::HashMap;

#[derive(IntuicioStruct, Default)]
#[intuicio(module_name = "http")]
pub struct HttpClient {
    #[intuicio(ignore)]
    url: String,
    #[intuicio(ignore)]
    status: u16,
    #[intuicio(ignore)]
    content: Option<Vec<u8>>,
}

#[intuicio_methods(module_name = "http")]
impl HttpClient {
    #[intuicio_method(use_registry)]
    pub fn get(registry: &Registry, url: Reference, query: Reference) -> Reference {
        let url = url.read::<Text>().unwrap();
        if let Ok(client) = Client::builder().build() {
            let mut request = client.get(url.as_str());
            if let Some(query) = query.read::<Map>() {
                request = request.query(
                    &query
                        .iter()
                        .map(|(key, value)| {
                            (key.as_str(), value.read::<Text>().unwrap().to_owned())
                        })
                        .collect::<HashMap<_, _>>(),
                );
            }
            if let Ok(response) = request.send() {
                return Reference::new(
                    HttpClient {
                        url: response.url().as_str().to_string(),
                        status: response.status().as_u16(),
                        content: response.bytes().ok().map(|bytes| bytes.to_vec()),
                    },
                    registry,
                );
            }
        }
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn post(
        registry: &Registry,
        url: Reference,
        query: Reference,
        body: Reference,
    ) -> Reference {
        let url = url.read::<Text>().unwrap();
        if let Ok(client) = Client::builder().build() {
            let mut request = client.post(url.as_str());
            if let Some(query) = query.read::<Map>() {
                let query = query
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.read::<Text>().unwrap().to_owned()))
                    .collect::<HashMap<_, _>>();
                request = request.query(&query);
            }
            if let Some(body) = body.read::<Text>() {
                request = request.body(body.to_string());
            } else if let Some(body) = body.read::<Bytes>() {
                request = request.body(body.get_ref().to_owned());
            }
            if let Ok(response) = request.send() {
                return Reference::new(
                    HttpClient {
                        url: response.url().as_str().to_string(),
                        status: response.status().as_u16(),
                        content: response.bytes().ok().map(|bytes| bytes.to_vec()),
                    },
                    registry,
                );
            }
        }
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn status(registry: &Registry, client: Reference) -> Reference {
        let client = client.read::<HttpClient>().unwrap();
        Reference::new_integer(client.status as Integer, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn url(registry: &Registry, client: Reference) -> Reference {
        let client = client.read::<HttpClient>().unwrap();
        Reference::new_text(client.url.to_owned(), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn bytes(registry: &Registry, client: Reference) -> Reference {
        let client = client.read::<HttpClient>().unwrap();
        client
            .content
            .as_ref()
            .map(|bytes| Reference::new(Bytes::new_raw(bytes.to_owned()), registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn text(registry: &Registry, client: Reference) -> Reference {
        let client = client.read::<HttpClient>().unwrap();
        client
            .content
            .as_ref()
            .map(|bytes| Reference::new_text(Text::from_utf8_lossy(bytes).to_string(), registry))
            .unwrap_or_default()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn version() -> IntuicioVersion {
    core_version()
}

#[unsafe(no_mangle)]
pub extern "C" fn install(registry: &mut Registry) {
    registry.add_type(HttpClient::define_struct(registry));
    registry.add_function(HttpClient::get__define_function(registry));
    registry.add_function(HttpClient::post__define_function(registry));
    registry.add_function(HttpClient::status__define_function(registry));
    registry.add_function(HttpClient::url__define_function(registry));
    registry.add_function(HttpClient::bytes__define_function(registry));
    registry.add_function(HttpClient::text__define_function(registry));
}
