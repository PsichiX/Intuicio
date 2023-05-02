use clap::Parser;
use intuicio_core::{
    function::FunctionBody,
    prelude::{Context, FunctionQuery},
    registry::Registry,
    script::{ScriptFunctionGenerator, ScriptHandle},
    struct_type::StructQuery,
    Visibility,
};
use intuicio_frontend_assembler::{AsmExpression, AsmFunction, AsmNodes, AsmStruct};
use intuicio_nodes::{
    nodes::ResponseSuggestionNode,
    server::{
        NodeGraphId, NodeGraphServer, NodeGraphServerError, RequestAdd, RequestQueryRegion,
        RequestRemove, RequestUpdate, ResponseQuery,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{net::TcpStream, thread::spawn};
use websocket::{
    sync::{Server, Writer},
    OwnedMessage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestFunctionItem {
    pub module_name: String,
    pub content: AsmFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestStructItem {
    pub module_name: String,
    pub content: AsmStruct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestFunctionQuery {
    pub name: String,
    pub module_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub struct_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestStructQuery {
    pub name: String,
    pub module_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Create {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
    },
    Destroy {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    List {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
    },
    Add {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
        content: RequestAdd<AsmNodes>,
    },
    Remove {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
        content: RequestRemove<AsmNodes>,
    },
    Update {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
        content: RequestUpdate<AsmNodes>,
    },
    Clear {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    QueryAll {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    QueryRegion {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
        content: RequestQueryRegion,
    },
    Serialize {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    Deserialize {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
        content: Value,
    },
    SuggestAllNodes {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        x: i64,
        y: i64,
    },
    RegistryAdd {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        structs: Vec<RequestStructItem>,
        functions: Vec<RequestFunctionItem>,
    },
    RegistryRemove {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        structs: Vec<RequestStructQuery>,
        functions: Vec<RequestFunctionQuery>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Error {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        error: NodeGraphServerError,
    },
    Create {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    Destroy {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    List {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graphs: Vec<NodeGraphId<AsmNodes>>,
    },
    Add {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    Remove {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    Update {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    Clear {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    QueryAll {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
        content: ResponseQuery<AsmNodes>,
    },
    QueryRegion {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
        content: ResponseQuery<AsmNodes>,
    },
    Serialize {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
        content: Value,
    },
    Deserialize {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        graph: NodeGraphId<AsmNodes>,
    },
    SuggestAllNodes {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
        x: i64,
        y: i64,
        content: Vec<ResponseSuggestionNode<AsmNodes>>,
    },
    RegistryAdd {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
    },
    RegistryRemove {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        payload: String,
    },
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Websocket server port number.
    #[arg(value_name = "INTEGER")]
    port: Option<u16>,

    /// Print graph state on each graph operation.
    #[arg(short, long)]
    verbose: bool,
}

macro_rules! send_message {
    ($sender:expr, $message:expr) => {{
        println!("* Send: {:?}", $message);
        let message = match serde_json::to_string(&$message) {
            Ok(message) => message,
            Err(error) => {
                println!("Could not stringify JSON message: {}", error);
                return ProcessRequestStatus::Skip;
            }
        };
        let _ = $sender.send_message(&OwnedMessage::Text(message));
    }};
}

macro_rules! try_log_graph_state {
    ($verbose:expr, $server:expr, $graph:expr) => {{
        if $verbose {
            if let Ok(graph) = $server.graph($graph) {
                println!("* Graph state: {:#?}", graph)
            }
        }
    }};
}

fn main() {
    let cli = Cli::parse();
    let port = cli.port.unwrap_or(8001);
    let address = format!("127.0.0.1:{}", port);
    let server =
        Server::bind(address).unwrap_or_else(|_| panic!("Unable to bind server to port: {port}"));
    println!("Started node graph server on port: {}", port);

    for request in server.filter_map(Result::ok) {
        spawn(move || {
            let client = match request.accept() {
                Ok(client) => client,
                Err((_, error)) => {
                    println!("New connection failed to be accepted: {}", error);
                    return;
                }
            };
            let (mut receiver, mut sender) = match client.split() {
                Ok(result) => result,
                Err(error) => {
                    println!(
                        "Could not get sender and receiver for new client: {}",
                        error
                    );
                    return;
                }
            };
            let mut registry = Registry::default().with_basic_types();
            let mut server = NodeGraphServer::<AsmNodes>::default();
            println!("New client connected");

            for message in receiver.incoming_messages() {
                let message = match message {
                    Ok(message) => message,
                    Err(error) => {
                        println!("Error decoding incomming message: {}", error);
                        continue;
                    }
                };

                match message {
                    OwnedMessage::Close(_) => {
                        let message = OwnedMessage::Close(None);
                        let _ = sender.send_message(&message);
                        println!("Client disconnected");
                        return;
                    }
                    OwnedMessage::Ping(ping) => {
                        let message = OwnedMessage::Pong(ping);
                        let _ = sender.send_message(&message);
                    }
                    OwnedMessage::Text(message) => {
                        match process_request(
                            &mut sender,
                            &message,
                            &mut server,
                            &mut registry,
                            cli.verbose,
                        ) {
                            ProcessRequestStatus::Skip => continue,
                            ProcessRequestStatus::Proceed => {}
                        }
                    }
                    OwnedMessage::Binary(bytes) => {
                        let message = String::from_utf8_lossy(&bytes);
                        match process_request(
                            &mut sender,
                            message.as_ref(),
                            &mut server,
                            &mut registry,
                            cli.verbose,
                        ) {
                            ProcessRequestStatus::Skip => continue,
                            ProcessRequestStatus::Proceed => {}
                        }
                    }
                    OwnedMessage::Pong(_) => {}
                }
            }
        });
    }
}

fn process_request(
    sender: &mut Writer<TcpStream>,
    message: &str,
    server: &mut NodeGraphServer<AsmNodes>,
    registry: &mut Registry,
    verbose: bool,
) -> ProcessRequestStatus {
    let message = match serde_json::from_str::<Request>(message) {
        Ok(message) => message,
        Err(error) => {
            println!("Could not parse JSON message: {}", error);
            return ProcessRequestStatus::Skip;
        }
    };
    println!("* Received: {:?}", message);
    match message {
        Request::Create { payload } => {
            let graph = server.create();
            let message = Response::Create { payload, graph };
            send_message!(sender, message);
        }
        Request::Destroy { payload, graph } => match server.destroy(graph) {
            Ok(_) => {
                let message = Response::Destroy { payload, graph };
                send_message!(sender, message);
            }
            Err(error) => {
                let message = Response::Error { payload, error };
                send_message!(sender, message);
            }
        },
        Request::List { payload } => {
            let graphs = server.list().cloned().collect();
            let message = Response::List { payload, graphs };
            send_message!(sender, message);
        }
        Request::Add {
            payload,
            graph,
            content,
        } => match server.add(graph, content, registry) {
            Ok(_) => {
                try_log_graph_state!(verbose, server, graph);
                let message = Response::Add { payload, graph };
                send_message!(sender, message);
            }
            Err(error) => {
                let message = Response::Error { payload, error };
                send_message!(sender, message);
            }
        },
        Request::Remove {
            payload,
            graph,
            content,
        } => match server.remove(graph, content, registry) {
            Ok(_) => {
                try_log_graph_state!(verbose, server, graph);
                let message = Response::Remove { payload, graph };
                send_message!(sender, message);
            }
            Err(error) => {
                let message = Response::Error { payload, error };
                send_message!(sender, message);
            }
        },
        Request::Update {
            payload,
            graph,
            content,
        } => match server.update(graph, content) {
            Ok(_) => {
                try_log_graph_state!(verbose, server, graph);
                let message = Response::Update { payload, graph };
                send_message!(sender, message);
            }
            Err(error) => {
                let message = Response::Error { payload, error };
                send_message!(sender, message);
            }
        },
        Request::Clear { payload, graph } => match server.clear(graph) {
            Ok(_) => {
                try_log_graph_state!(verbose, server, graph);
                let message = Response::Clear { payload, graph };
                send_message!(sender, message);
            }
            Err(error) => {
                let message = Response::Error { payload, error };
                send_message!(sender, message);
            }
        },
        Request::QueryAll { payload, graph } => match server.query_all(graph) {
            Ok(content) => {
                let message = Response::QueryAll {
                    payload,
                    graph,
                    content,
                };
                send_message!(sender, message);
            }
            Err(error) => {
                let message = Response::Error { payload, error };
                send_message!(sender, message);
            }
        },
        Request::QueryRegion {
            payload,
            graph,
            content,
        } => match server.query_region(graph, content) {
            Ok(content) => {
                let message = Response::QueryRegion {
                    payload,
                    graph,
                    content,
                };
                send_message!(sender, message);
            }
            Err(error) => {
                let message = Response::Error { payload, error };
                send_message!(sender, message);
            }
        },
        Request::Serialize { payload, graph } => match server.graph(graph) {
            Ok(content) => {
                let message = Response::Serialize {
                    payload,
                    graph,
                    content: serde_json::to_value(content).unwrap(),
                };
                send_message!(sender, message);
            }
            Err(error) => {
                let message = Response::Error { payload, error };
                send_message!(sender, message);
            }
        },
        Request::Deserialize {
            payload,
            graph,
            content,
        } => match server.graph_mut(graph) {
            Ok(found) => {
                *found = serde_json::from_value(content).unwrap();
                let message = Response::Deserialize { payload, graph };
                send_message!(sender, message);
            }
            Err(error) => {
                let message = Response::Error { payload, error };
                send_message!(sender, message);
            }
        },
        Request::SuggestAllNodes { payload, x, y } => {
            let message = Response::SuggestAllNodes {
                payload,
                x,
                y,
                content: NodeGraphServer::<AsmNodes>::suggest_all_nodes(x, y, registry),
            };
            send_message!(sender, message);
        }
        Request::RegistryAdd {
            payload,
            structs,
            functions,
        } => {
            for data in structs {
                data.content.compile(&data.module_name).install(registry);
            }
            for data in functions {
                data.content
                    .compile(&data.module_name)
                    .install::<EmptyScriptFunctionGenerator>(registry, ());
            }
            let message = Response::RegistryAdd { payload };
            send_message!(sender, message);
        }
        Request::RegistryRemove {
            payload,
            structs,
            functions,
        } => {
            for data in structs {
                if let Some(handle) = registry.find_struct(StructQuery {
                    name: Some(data.name.into()),
                    module_name: Some(data.module_name.into()),
                    visibility: data.visibility,
                    ..Default::default()
                }) {
                    registry.remove_struct(handle);
                }
            }
            for data in functions {
                if let Some(handle) = registry.find_function(FunctionQuery {
                    name: Some(data.name.into()),
                    module_name: Some(data.module_name.to_owned().into()),
                    struct_query: data.struct_name.map(|name| StructQuery {
                        name: Some(name.into()),
                        module_name: Some(data.module_name.into()),
                        ..Default::default()
                    }),
                    visibility: data.visibility,
                    ..Default::default()
                }) {
                    registry.remove_function(handle);
                }
            }
            let message = Response::RegistryAdd { payload };
            send_message!(sender, message);
        }
    }
    ProcessRequestStatus::Proceed
}

enum ProcessRequestStatus {
    Skip,
    Proceed,
}

struct EmptyScriptFunctionGenerator;

impl ScriptFunctionGenerator<AsmExpression> for EmptyScriptFunctionGenerator {
    type Input = ();
    type Output = ();

    fn generate_function_body(
        _: ScriptHandle<'static, AsmExpression>,
        _: Self::Input,
    ) -> Option<(FunctionBody, Self::Output)> {
        Some((FunctionBody::Pointer(empty_function_body), ()))
    }
}

fn empty_function_body(_: &mut Context, _: &Registry) {}
