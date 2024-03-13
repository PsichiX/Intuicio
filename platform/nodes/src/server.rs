use crate::nodes::*;
use intuicio_core::registry::Registry;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
};
use typid::ID;

pub type NodeGraphId<T> = ID<NodeGraph<T>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestAdd<T: NodeDefinition> {
    pub nodes: Vec<Node<T>>,
    pub connections: Vec<NodeConnection<T>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRemove<T: NodeDefinition> {
    pub nodes: Vec<NodeId<T>>,
    pub connections: Vec<NodeConnection<T>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestUpdate<T: NodeDefinition> {
    pub nodes: Vec<Node<T>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestQueryRegion {
    pub fx: i64,
    pub fy: i64,
    pub tx: i64,
    pub ty: i64,
    pub extrude: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseQuery<T: NodeDefinition> {
    pub nodes: Vec<Node<T>>,
    pub connections: Vec<NodeConnection<T>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeGraphServerError {
    NodeGraphDoesNotExists(String),
    NodeNotFound { graph: String, node: String },
    ValidationErrors { graph: String, errors: Vec<String> },
}

impl std::fmt::Display for NodeGraphServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeGraphServerError::NodeGraphDoesNotExists(id) => {
                write!(f, "Node graph does not exists: {}", id)
            }
            NodeGraphServerError::NodeNotFound { graph, node } => {
                write!(f, "Node graph: {} does not have node: {}", graph, node)
            }
            NodeGraphServerError::ValidationErrors { graph, errors } => {
                write!(f, "Node graph: {} validation errors:", graph)?;
                for error in errors {
                    write!(f, "{}", error)?;
                }
                Ok(())
            }
        }
    }
}

impl Error for NodeGraphServerError {}

pub struct NodeGraphServer<T: NodeDefinition + Clone> {
    graphs: HashMap<NodeGraphId<T>, NodeGraph<T>>,
}

impl<T: NodeDefinition + Clone> Default for NodeGraphServer<T> {
    fn default() -> Self {
        Self {
            graphs: Default::default(),
        }
    }
}

impl<T: NodeDefinition + Clone> NodeGraphServer<T> {
    pub fn graph(&self, id: NodeGraphId<T>) -> Result<&NodeGraph<T>, NodeGraphServerError> {
        self.graphs
            .get(&id)
            .ok_or_else(|| NodeGraphServerError::NodeGraphDoesNotExists(id.to_string()))
    }

    pub fn graph_mut(
        &mut self,
        id: NodeGraphId<T>,
    ) -> Result<&mut NodeGraph<T>, NodeGraphServerError> {
        self.graphs
            .get_mut(&id)
            .ok_or_else(|| NodeGraphServerError::NodeGraphDoesNotExists(id.to_string()))
    }

    pub fn create(&mut self) -> NodeGraphId<T> {
        let id = NodeGraphId::new();
        self.graphs.insert(id, NodeGraph::default());
        id
    }

    pub fn destroy(&mut self, id: NodeGraphId<T>) -> Result<NodeGraph<T>, NodeGraphServerError> {
        self.graphs
            .remove(&id)
            .ok_or_else(|| NodeGraphServerError::NodeGraphDoesNotExists(id.to_string()))
    }

    pub fn list(&self) -> impl Iterator<Item = &NodeGraphId<T>> {
        self.graphs.keys()
    }

    pub fn add(
        &mut self,
        id: NodeGraphId<T>,
        request: RequestAdd<T>,
        registry: &Registry,
    ) -> Result<(), NodeGraphServerError> {
        if let Some(graph) = self.graphs.get_mut(&id) {
            for node in request.nodes {
                graph.add_node(node, registry);
            }
            for connection in request.connections {
                graph.connect_nodes(connection);
            }
            graph.refresh_spatial_cache();
            Ok(())
        } else {
            Err(NodeGraphServerError::NodeGraphDoesNotExists(id.to_string()))
        }
    }

    pub fn remove(
        &mut self,
        id: NodeGraphId<T>,
        request: RequestRemove<T>,
        registry: &Registry,
    ) -> Result<(), NodeGraphServerError> {
        if let Some(graph) = self.graphs.get_mut(&id) {
            for connection in request.connections {
                graph.disconnect_nodes(
                    connection.from_node,
                    connection.to_node,
                    &connection.from_pin,
                    &connection.to_pin,
                );
            }
            for id in request.nodes {
                graph.remove_node(id, registry);
            }
            graph.refresh_spatial_cache();
            Ok(())
        } else {
            Err(NodeGraphServerError::NodeGraphDoesNotExists(id.to_string()))
        }
    }

    pub fn update(
        &mut self,
        id: NodeGraphId<T>,
        request: RequestUpdate<T>,
    ) -> Result<(), NodeGraphServerError> {
        if let Some(graph) = self.graphs.get_mut(&id) {
            for source in &request.nodes {
                if graph.node(source.id()).is_none() {
                    return Err(NodeGraphServerError::NodeNotFound {
                        graph: id.to_string(),
                        node: source.id().to_string(),
                    });
                }
            }
            for source in request.nodes {
                let id = source.id();
                *graph.node_mut(id).unwrap() = source;
            }
            graph.refresh_spatial_cache();
            Ok(())
        } else {
            Err(NodeGraphServerError::NodeGraphDoesNotExists(id.to_string()))
        }
    }

    pub fn clear(&mut self, id: NodeGraphId<T>) -> Result<(), NodeGraphServerError> {
        if let Some(graph) = self.graphs.get_mut(&id) {
            graph.clear();
            graph.refresh_spatial_cache();
            Ok(())
        } else {
            Err(NodeGraphServerError::NodeGraphDoesNotExists(id.to_string()))
        }
    }

    pub fn query_all(
        &self,
        graph: NodeGraphId<T>,
    ) -> Result<ResponseQuery<T>, NodeGraphServerError> {
        if let Some(graph) = self.graphs.get(&graph) {
            Ok(ResponseQuery {
                nodes: graph.nodes().cloned().collect(),
                connections: graph.connections().cloned().collect(),
            })
        } else {
            Err(NodeGraphServerError::NodeGraphDoesNotExists(
                graph.to_string(),
            ))
        }
    }

    pub fn query_region(
        &self,
        graph: NodeGraphId<T>,
        request: RequestQueryRegion,
    ) -> Result<ResponseQuery<T>, NodeGraphServerError> {
        if let Some(graph) = self.graphs.get(&graph) {
            let RequestQueryRegion {
                fx,
                fy,
                tx,
                ty,
                extrude,
            } = request;
            let nodes = graph
                .query_region_nodes(fx, fy, tx, ty, extrude)
                .filter_map(|id| graph.node(id))
                .cloned()
                .collect::<Vec<_>>();
            let connections = nodes
                .iter()
                .flat_map(|node| graph.node_connections(node.id()))
                .cloned()
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            Ok(ResponseQuery { nodes, connections })
        } else {
            Err(NodeGraphServerError::NodeGraphDoesNotExists(
                graph.to_string(),
            ))
        }
    }

    pub fn suggest_all_nodes(
        x: i64,
        y: i64,
        registry: &Registry,
    ) -> Vec<ResponseSuggestionNode<T>> {
        NodeGraph::suggest_all_nodes(x, y, registry)
    }

    pub fn validate(
        &self,
        graph: NodeGraphId<T>,
        registry: &Registry,
    ) -> Result<(), NodeGraphServerError> {
        if let Some(item) = self.graphs.get(&graph) {
            match item.validate(registry) {
                Ok(_) => Ok(()),
                Err(errors) => Err(NodeGraphServerError::ValidationErrors {
                    graph: graph.to_string(),
                    errors: errors.into_iter().map(|error| error.to_string()).collect(),
                }),
            }
        } else {
            Err(NodeGraphServerError::NodeGraphDoesNotExists(
                graph.to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use intuicio_core::prelude::*;
    use serde::{de::DeserializeOwned, Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TypeInfo;

    impl std::fmt::Display for TypeInfo {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "")
        }
    }

    impl NodeTypeInfo for TypeInfo {
        fn type_query(&self) -> TypeQuery {
            Default::default()
        }

        fn are_compatible(&self, _: &Self) -> bool {
            true
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum Nodes {
        Start,
        Expression(i32),
        Result,
        Convert(String),
    }

    impl NodeDefinition for Nodes {
        type TypeInfo = TypeInfo;

        fn node_label(&self, _: &Registry) -> String {
            format!("{:?}", self)
        }

        fn node_pins_in(&self, _: &Registry) -> Vec<NodePin<Self::TypeInfo>> {
            match self {
                Nodes::Start => vec![],
                Nodes::Expression(_) => vec![NodePin::property("Value")],
                Nodes::Result => vec![
                    NodePin::execute("In", false),
                    NodePin::parameter("Data", TypeInfo),
                ],
                Nodes::Convert(_) => vec![
                    NodePin::execute("In", false),
                    NodePin::property("Name"),
                    NodePin::parameter("Data in", TypeInfo),
                ],
            }
        }

        fn node_pins_out(&self, _: &Registry) -> Vec<NodePin<Self::TypeInfo>> {
            match self {
                Nodes::Start => vec![NodePin::execute("Out", false)],
                Nodes::Expression(_) => vec![NodePin::parameter("Data", TypeInfo)],
                Nodes::Result => vec![],
                Nodes::Convert(_) => vec![
                    NodePin::execute("Out", false),
                    NodePin::parameter("Data out", TypeInfo),
                ],
            }
        }

        fn node_is_start(&self, _: &Registry) -> bool {
            matches!(self, Self::Start)
        }

        fn node_suggestions(
            _: i64,
            _: i64,
            _: NodeSuggestion<Self>,
            _: &Registry,
        ) -> Vec<ResponseSuggestionNode<Self>> {
            vec![]
        }
    }

    fn mock_transfer<T: Serialize + DeserializeOwned>(value: T) -> T {
        let content = serde_json::to_string(&value).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    #[test]
    fn test_server() {
        let registry = Registry::default().with_basic_types();
        let mut server = NodeGraphServer::default();
        let graph = server.create();
        let start = Node::new(0, 0, Nodes::Start);
        let expression = Node::new(0, 0, Nodes::Expression(42));
        let convert = Node::new(0, 0, Nodes::Convert("foo".to_owned()));
        let result = Node::new(0, 0, Nodes::Result);
        server
            .add(
                graph,
                mock_transfer(RequestAdd {
                    connections: vec![
                        NodeConnection::new(start.id(), convert.id(), "Out", "In"),
                        NodeConnection::new(convert.id(), result.id(), "Out", "In"),
                        NodeConnection::new(expression.id(), convert.id(), "Data", "Data in"),
                    ],
                    nodes: vec![
                        start.clone(),
                        expression.clone(),
                        convert.clone(),
                        result.clone(),
                    ],
                }),
                &registry,
            )
            .unwrap();
        let temp = server.query_all(graph).unwrap();
        assert_eq!(temp.nodes.len(), 4);
        assert_eq!(temp.connections.len(), 3);
        server
            .remove(
                graph,
                RequestRemove {
                    nodes: vec![result.id(), convert.id()],
                    connections: vec![],
                },
                &registry,
            )
            .unwrap();
        let temp = server.query_all(graph).unwrap();
        assert_eq!(temp.nodes.len(), 2);
        assert_eq!(temp.connections.len(), 0);
        assert!(matches!(
            server.update(
                graph,
                mock_transfer(RequestUpdate {
                    nodes: vec![expression.clone(), convert.clone()],
                }),
            ),
            Err(NodeGraphServerError::NodeNotFound { .. })
        ));
        let temp = server.query_all(graph).unwrap();
        assert_eq!(temp.nodes.len(), 2);
        assert_eq!(temp.connections.len(), 0);
        server
            .update(
                graph,
                mock_transfer(RequestUpdate {
                    nodes: vec![expression.clone(), start.clone()],
                }),
            )
            .unwrap();
        let temp = server.query_all(graph).unwrap();
        assert_eq!(temp.nodes.len(), 2);
        assert_eq!(temp.connections.len(), 0);
    }
}
