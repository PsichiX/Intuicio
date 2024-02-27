use intuicio_core::{registry::Registry, struct_type::StructQuery};
use rstar::{Envelope, Point, PointDistance, RTree, RTreeObject, AABB};
use serde::{Deserialize, Serialize};
use serde_intermediate::{
    de::intermediate::DeserializeMode, error::Result as IntermediateResult, Intermediate,
};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Display,
    hash::{Hash, Hasher},
};
use typid::ID;

pub type NodeId<T> = ID<Node<T>>;
pub type PropertyCastMode = DeserializeMode;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PropertyValue {
    value: Intermediate,
}

impl PropertyValue {
    pub fn new<T: Serialize>(value: &T) -> IntermediateResult<Self> {
        Ok(Self {
            value: serde_intermediate::to_intermediate(value)?,
        })
    }

    pub fn get<'a, T: Deserialize<'a>>(&'a self, mode: PropertyCastMode) -> IntermediateResult<T> {
        serde_intermediate::from_intermediate_as(&self.value, mode)
    }

    pub fn get_exact<'a, T: Deserialize<'a>>(&'a self) -> IntermediateResult<T> {
        self.get(PropertyCastMode::Exact)
    }

    pub fn get_interpret<'a, T: Deserialize<'a>>(&'a self) -> IntermediateResult<T> {
        self.get(PropertyCastMode::Interpret)
    }

    pub fn into_inner(self) -> Intermediate {
        self.value
    }
}

pub trait NodeTypeInfo:
    Clone + std::fmt::Debug + Display + PartialEq + Serialize + for<'de> Deserialize<'de>
{
    fn struct_query(&self) -> StructQuery;
    fn are_compatible(&self, other: &Self) -> bool;
}

pub trait NodeDefinition: Sized {
    type TypeInfo: NodeTypeInfo;

    fn node_label(&self, registry: &Registry) -> String;
    fn node_pins_in(&self, registry: &Registry) -> Vec<NodePin<Self::TypeInfo>>;
    fn node_pins_out(&self, registry: &Registry) -> Vec<NodePin<Self::TypeInfo>>;
    fn node_is_start(&self, registry: &Registry) -> bool;
    fn node_suggestions(
        x: i64,
        y: i64,
        suggestion: NodeSuggestion<Self>,
        registry: &Registry,
    ) -> Vec<ResponseSuggestionNode<Self>>;

    #[allow(unused_variables)]
    fn validate_connection(
        &self,
        source: &Self,
        registry: &Registry,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn get_property(&self, name: &str) -> Option<PropertyValue> {
        None
    }

    #[allow(unused_variables)]
    fn set_property(&mut self, name: &str, value: PropertyValue) {}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound = "TI: NodeTypeInfo")]
pub enum NodePin<TI: NodeTypeInfo> {
    Execute { name: String, subscope: bool },
    Parameter { name: String, type_info: TI },
    Property { name: String },
}

impl<TI: NodeTypeInfo> NodePin<TI> {
    pub fn execute(name: impl ToString, subscope: bool) -> Self {
        Self::Execute {
            name: name.to_string(),
            subscope,
        }
    }

    pub fn parameter(name: impl ToString, type_info: TI) -> Self {
        Self::Parameter {
            name: name.to_string(),
            type_info,
        }
    }

    pub fn property(name: impl ToString) -> Self {
        Self::Property {
            name: name.to_string(),
        }
    }

    pub fn is_execute(&self) -> bool {
        matches!(self, Self::Execute { .. })
    }

    pub fn is_parameter(&self) -> bool {
        matches!(self, Self::Parameter { .. })
    }

    pub fn is_property(&self) -> bool {
        matches!(self, Self::Property { .. })
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Execute { name, .. }
            | Self::Parameter { name, .. }
            | Self::Property { name, .. } => name,
        }
    }

    pub fn has_subscope(&self) -> bool {
        matches!(self, Self::Execute { subscope: true, .. })
    }

    pub fn type_info(&self) -> Option<&TI> {
        match self {
            Self::Parameter { type_info, .. } => Some(type_info),
            _ => None,
        }
    }
}

pub enum NodeSuggestion<'a, T: NodeDefinition> {
    All,
    NodeInputPin(&'a T, &'a NodePin<T::TypeInfo>),
    NodeOutputPin(&'a T, &'a NodePin<T::TypeInfo>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseSuggestionNode<T: NodeDefinition> {
    pub category: String,
    pub label: String,
    pub node: Node<T>,
}

impl<T: NodeDefinition> ResponseSuggestionNode<T> {
    pub fn new(category: impl ToString, node: Node<T>, registry: &Registry) -> Self {
        Self {
            category: category.to_string(),
            label: node.data.node_label(registry),
            node,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node<T: NodeDefinition> {
    id: NodeId<T>,
    pub x: i64,
    pub y: i64,
    pub data: T,
}

impl<T: NodeDefinition> Node<T> {
    pub fn new(x: i64, y: i64, data: T) -> Self {
        Self {
            id: Default::default(),
            x,
            y,
            data,
        }
    }

    pub fn id(&self) -> NodeId<T> {
        self.id
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NodeConnection<T: NodeDefinition> {
    pub from_node: NodeId<T>,
    pub to_node: NodeId<T>,
    pub from_pin: String,
    pub to_pin: String,
}

impl<T: NodeDefinition> NodeConnection<T> {
    pub fn new(from_node: NodeId<T>, to_node: NodeId<T>, from_pin: &str, to_pin: &str) -> Self {
        Self {
            from_node,
            to_node,
            from_pin: from_pin.to_owned(),
            to_pin: to_pin.to_owned(),
        }
    }
}

impl<T: NodeDefinition> std::fmt::Debug for NodeConnection<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeConnection")
            .field("from_node", &self.from_node)
            .field("to_node", &self.to_node)
            .field("from_pin", &self.from_pin)
            .field("to_pin", &self.to_pin)
            .finish()
    }
}

impl<T: NodeDefinition> PartialEq for NodeConnection<T> {
    fn eq(&self, other: &Self) -> bool {
        self.from_node == other.from_node
            && self.to_node == other.to_node
            && self.from_pin == other.from_pin
            && self.to_pin == other.to_pin
    }
}

impl<T: NodeDefinition> Eq for NodeConnection<T> {}

impl<T: NodeDefinition> Hash for NodeConnection<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.from_node.hash(state);
        self.to_node.hash(state);
        self.from_pin.hash(state);
        self.to_pin.hash(state);
    }
}

#[derive(Debug)]
pub enum ConnectionError {
    InternalConnection(String),
    SourceNodeNotFound(String),
    TargetNodeNotFound(String),
    NodesNotFound {
        from: String,
        to: String,
    },
    SourcePinNotFound {
        node: String,
        pin: String,
    },
    TargetPinNotFound {
        node: String,
        pin: String,
    },
    MismatchTypes {
        from_node: String,
        from_pin: String,
        from_type_info: String,
        to_node: String,
        to_pin: String,
        to_type_info: String,
    },
    MismatchPins {
        from_node: String,
        from_pin: String,
        to_node: String,
        to_pin: String,
    },
    CycleNodeFound(String),
    Custom(Box<dyn Error>),
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InternalConnection(node) => {
                write!(f, "Trying to connect node: {} to itself", node)
            }
            Self::SourceNodeNotFound(node) => write!(f, "Source node: {} not found", node),
            Self::TargetNodeNotFound(node) => write!(f, "Target node: {} not found", node),
            Self::NodesNotFound { from, to } => {
                write!(f, "Source: {} and target: {} nodes not found", from, to)
            }
            Self::SourcePinNotFound { node, pin } => {
                write!(f, "Source pin: {} for node: {} not found", pin, node)
            }
            Self::TargetPinNotFound { node, pin } => {
                write!(f, "Target pin: {} for node: {} not found", pin, node)
            }
            Self::MismatchTypes {
                from_node,
                from_pin,
                from_type_info,
                to_node,
                to_pin,
                to_type_info,
            } => {
                write!(
                    f,
                    "Source type: {} of pin: {} for node: {} does not match target type: {} of pin: {} for node: {}",
                    from_type_info, from_pin, from_node, to_type_info, to_pin, to_node
                )
            }
            Self::MismatchPins {
                from_node,
                from_pin,
                to_node,
                to_pin,
            } => {
                write!(
                    f,
                    "Source pin: {} kind for node: {} does not match target pin: {} kind for node: {}",
                    from_pin, from_node, to_pin, to_node
                )
            }
            Self::CycleNodeFound(node) => write!(f, "Found cycle node: {}", node),
            Self::Custom(error) => error.fmt(f),
        }
    }
}

impl Error for ConnectionError {}

#[derive(Debug)]
pub enum NodeGraphError {
    Connection(ConnectionError),
    DuplicateFunctionInputNames(String),
    DuplicateFunctionOutputNames(String),
}

impl std::fmt::Display for NodeGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connection(connection) => connection.fmt(f),
            Self::DuplicateFunctionInputNames(name) => {
                write!(
                    f,
                    "Found duplicate `{}` function input with different types",
                    name
                )
            }
            Self::DuplicateFunctionOutputNames(name) => {
                write!(
                    f,
                    "Found duplicate `{}` function output with different types",
                    name
                )
            }
        }
    }
}

impl Error for NodeGraphError {}

#[derive(Clone)]
struct SpatialNode<T: NodeDefinition> {
    id: NodeId<T>,
    x: i64,
    y: i64,
}

impl<T: NodeDefinition> RTreeObject for SpatialNode<T> {
    type Envelope = AABB<[i64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.x, self.y])
    }
}

impl<T: NodeDefinition> PointDistance for SpatialNode<T> {
    fn distance_2(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
    ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar {
        let dx = self.x - point[0];
        let dy = self.y - point[1];
        dx * dx + dy * dy
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NodeGraph<T: NodeDefinition> {
    nodes: Vec<Node<T>>,
    connections: Vec<NodeConnection<T>>,
    #[serde(skip, default)]
    rtree: RTree<SpatialNode<T>>,
}

impl<T: NodeDefinition> Default for NodeGraph<T> {
    fn default() -> Self {
        Self {
            nodes: vec![],
            connections: vec![],
            rtree: Default::default(),
        }
    }
}

impl<T: NodeDefinition> NodeGraph<T> {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.connections.clear();
    }

    pub fn refresh_spatial_cache(&mut self) {
        self.rtree = RTree::bulk_load(
            self.nodes
                .iter()
                .map(|node| SpatialNode {
                    id: node.id,
                    x: node.x,
                    y: node.y,
                })
                .collect(),
        );
    }

    pub fn query_nearest_nodes(&self, x: i64, y: i64) -> impl Iterator<Item = NodeId<T>> + '_ {
        self.rtree
            .nearest_neighbor_iter(&[x, y])
            .map(|node| node.id)
    }

    pub fn query_region_nodes(
        &self,
        fx: i64,
        fy: i64,
        tx: i64,
        ty: i64,
        extrude: i64,
    ) -> impl Iterator<Item = NodeId<T>> + '_ {
        self.rtree
            .locate_in_envelope(&AABB::from_corners(
                [fx - extrude, fy - extrude],
                [tx - extrude, ty - extrude],
            ))
            .map(|node| node.id)
    }

    pub fn suggest_all_nodes(
        x: i64,
        y: i64,
        registry: &Registry,
    ) -> Vec<ResponseSuggestionNode<T>> {
        T::node_suggestions(x, y, NodeSuggestion::All, registry)
    }

    pub fn suggest_node_input_pin(
        &self,
        x: i64,
        y: i64,
        id: NodeId<T>,
        name: &str,
        registry: &Registry,
    ) -> Vec<ResponseSuggestionNode<T>> {
        if let Some(node) = self.node(id) {
            if let Some(pin) = node
                .data
                .node_pins_in(registry)
                .into_iter()
                .find(|pin| pin.name() == name)
            {
                return T::node_suggestions(
                    x,
                    y,
                    NodeSuggestion::NodeInputPin(&node.data, &pin),
                    registry,
                );
            }
        }
        vec![]
    }

    pub fn suggest_node_output_pin(
        &self,
        x: i64,
        y: i64,
        id: NodeId<T>,
        name: &str,
        registry: &Registry,
    ) -> Vec<ResponseSuggestionNode<T>> {
        if let Some(node) = self.node(id) {
            if let Some(pin) = node
                .data
                .node_pins_out(registry)
                .into_iter()
                .find(|pin| pin.name() == name)
            {
                return T::node_suggestions(
                    x,
                    y,
                    NodeSuggestion::NodeOutputPin(&node.data, &pin),
                    registry,
                );
            }
        }
        vec![]
    }

    pub fn node(&self, id: NodeId<T>) -> Option<&Node<T>> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn node_mut(&mut self, id: NodeId<T>) -> Option<&mut Node<T>> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node<T>> {
        self.nodes.iter()
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node<T>> {
        self.nodes.iter_mut()
    }

    pub fn add_node(&mut self, node: Node<T>, registry: &Registry) -> Option<NodeId<T>> {
        if node.data.node_is_start(registry)
            && self
                .nodes
                .iter()
                .any(|node| node.data.node_is_start(registry))
        {
            return None;
        }
        let id = node.id;
        if let Some(index) = self.nodes.iter().position(|node| node.id == id) {
            self.nodes.swap_remove(index);
        }
        self.nodes.push(node);
        Some(id)
    }

    pub fn remove_node(&mut self, id: NodeId<T>, registry: &Registry) -> Option<Node<T>> {
        if let Some(index) = self
            .nodes
            .iter()
            .position(|node| node.id == id && !node.data.node_is_start(registry))
        {
            self.disconnect_node(id, None);
            Some(self.nodes.swap_remove(index))
        } else {
            None
        }
    }

    pub fn connect_nodes(&mut self, connection: NodeConnection<T>) {
        if !self.connections.iter().any(|other| &connection == other) {
            self.disconnect_node(connection.from_node, Some(&connection.from_pin));
            self.disconnect_node(connection.to_node, Some(&connection.to_pin));
            self.connections.push(connection);
        }
    }

    pub fn disconnect_nodes(
        &mut self,
        from_node: NodeId<T>,
        to_node: NodeId<T>,
        from_pin: &str,
        to_pin: &str,
    ) {
        if let Some(index) = self.connections.iter().position(|connection| {
            connection.from_node == from_node
                && connection.to_node == to_node
                && connection.from_pin == from_pin
                && connection.to_pin == to_pin
        }) {
            self.connections.swap_remove(index);
        }
    }

    pub fn disconnect_node(&mut self, node: NodeId<T>, pin: Option<&str>) {
        let to_remove = self
            .connections
            .iter()
            .enumerate()
            .filter_map(|(index, connection)| {
                if let Some(pin) = pin {
                    if connection.from_node == node && connection.from_pin == pin {
                        return Some(index);
                    }
                    if connection.to_node == node && connection.to_pin == pin {
                        return Some(index);
                    }
                } else if connection.from_node == node || connection.to_node == node {
                    return Some(index);
                }
                None
            })
            .collect::<Vec<_>>();
        for index in to_remove.into_iter().rev() {
            self.connections.swap_remove(index);
        }
    }

    pub fn connections(&self) -> impl Iterator<Item = &NodeConnection<T>> {
        self.connections.iter()
    }

    pub fn node_connections(&self, id: NodeId<T>) -> impl Iterator<Item = &NodeConnection<T>> {
        self.connections
            .iter()
            .filter(move |connection| connection.from_node == id || connection.to_node == id)
    }

    pub fn node_connections_in<'a>(
        &'a self,
        id: NodeId<T>,
        pin: Option<&'a str>,
    ) -> impl Iterator<Item = &NodeConnection<T>> + 'a {
        self.connections.iter().filter(move |connection| {
            connection.to_node == id && pin.map(|pin| connection.to_pin == pin).unwrap_or(true)
        })
    }

    pub fn node_connections_out<'a>(
        &'a self,
        id: NodeId<T>,
        pin: Option<&'a str>,
    ) -> impl Iterator<Item = &NodeConnection<T>> + 'a {
        self.connections.iter().filter(move |connection| {
            connection.from_node == id && pin.map(|pin| connection.from_pin == pin).unwrap_or(true)
        })
    }

    pub fn node_neighbors_in<'a>(
        &'a self,
        id: NodeId<T>,
        pin: Option<&'a str>,
    ) -> impl Iterator<Item = NodeId<T>> + 'a {
        self.node_connections_in(id, pin)
            .map(move |connection| connection.from_node)
    }

    pub fn node_neighbors_out<'a>(
        &'a self,
        id: NodeId<T>,
        pin: Option<&'a str>,
    ) -> impl Iterator<Item = NodeId<T>> + 'a {
        self.node_connections_out(id, pin)
            .map(move |connection| connection.to_node)
    }

    pub fn validate(&self, registry: &Registry) -> Result<(), Vec<NodeGraphError>> {
        let mut errors = self
            .connections
            .iter()
            .filter_map(|connection| self.validate_connection(connection, registry))
            .map(NodeGraphError::Connection)
            .collect::<Vec<_>>();
        if let Some(error) = self.detect_cycles() {
            errors.push(NodeGraphError::Connection(error));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_connection(
        &self,
        connection: &NodeConnection<T>,
        registry: &Registry,
    ) -> Option<ConnectionError> {
        if connection.from_node == connection.to_node {
            return Some(ConnectionError::InternalConnection(
                connection.from_node.to_string(),
            ));
        }
        let from = self
            .nodes
            .iter()
            .find(|node| node.id == connection.from_node);
        let to = self.nodes.iter().find(|node| node.id == connection.to_node);
        let (from_node, to_node) = match (from, to) {
            (Some(from), Some(to)) => (from, to),
            (Some(_), None) => {
                return Some(ConnectionError::TargetNodeNotFound(
                    connection.to_node.to_string(),
                ));
            }
            (None, Some(_)) => {
                return Some(ConnectionError::SourceNodeNotFound(
                    connection.from_node.to_string(),
                ));
            }
            (None, None) => {
                return Some(ConnectionError::NodesNotFound {
                    from: connection.from_node.to_string(),
                    to: connection.to_node.to_string(),
                });
            }
        };
        let from_pins_out = from_node.data.node_pins_out(registry);
        let from_pin = match from_pins_out
            .iter()
            .find(|pin| pin.name() == connection.from_pin)
        {
            Some(pin) => pin,
            None => {
                return Some(ConnectionError::SourcePinNotFound {
                    node: connection.from_node.to_string(),
                    pin: connection.from_pin.to_owned(),
                })
            }
        };
        let to_pins_in = to_node.data.node_pins_in(registry);
        let to_pin = match to_pins_in
            .iter()
            .find(|pin| pin.name() == connection.to_pin)
        {
            Some(pin) => pin,
            None => {
                return Some(ConnectionError::TargetPinNotFound {
                    node: connection.to_node.to_string(),
                    pin: connection.to_pin.to_owned(),
                })
            }
        };
        match (from_pin, to_pin) {
            (NodePin::Execute { .. }, NodePin::Execute { .. }) => {}
            (NodePin::Parameter { type_info: a, .. }, NodePin::Parameter { type_info: b, .. }) => {
                if !a.are_compatible(b) {
                    return Some(ConnectionError::MismatchTypes {
                        from_node: connection.from_node.to_string(),
                        from_pin: connection.from_pin.to_owned(),
                        to_node: connection.to_node.to_string(),
                        to_pin: connection.to_pin.to_owned(),
                        from_type_info: a.to_string(),
                        to_type_info: b.to_string(),
                    });
                }
            }
            (NodePin::Property { .. }, NodePin::Property { .. }) => {}
            _ => {
                return Some(ConnectionError::MismatchPins {
                    from_node: connection.from_node.to_string(),
                    from_pin: connection.from_pin.to_owned(),
                    to_node: connection.to_node.to_string(),
                    to_pin: connection.to_pin.to_owned(),
                });
            }
        }
        if let Err(error) = to_node.data.validate_connection(&from_node.data, registry) {
            return Some(ConnectionError::Custom(error));
        }
        None
    }

    fn detect_cycles(&self) -> Option<ConnectionError> {
        let mut visited = HashSet::with_capacity(self.nodes.len());
        let mut available = self.nodes.iter().map(|node| node.id).collect::<Vec<_>>();
        while let Some(id) = available.first() {
            if let Some(error) = self.detect_cycle(*id, &mut available, &mut visited) {
                return Some(error);
            }
            available.swap_remove(0);
        }
        None
    }

    fn detect_cycle(
        &self,
        id: NodeId<T>,
        available: &mut Vec<NodeId<T>>,
        visited: &mut HashSet<NodeId<T>>,
    ) -> Option<ConnectionError> {
        if visited.contains(&id) {
            return Some(ConnectionError::CycleNodeFound(id.to_string()));
        }
        visited.insert(id);
        for id in self.node_neighbors_out(id, None) {
            if let Some(index) = available.iter().position(|item| item == &id) {
                available.swap_remove(index);
                if let Some(error) = self.detect_cycle(id, available, visited) {
                    return Some(error);
                }
            }
        }
        None
    }

    pub fn visit<V: NodeGraphVisitor<T>>(
        &self,
        visitor: &mut V,
        registry: &Registry,
    ) -> Vec<V::Output> {
        let starts = self
            .nodes
            .iter()
            .filter(|node| node.data.node_is_start(registry))
            .map(|node| node.id)
            .collect::<HashSet<_>>();
        let mut result = Vec::with_capacity(self.nodes.len());
        for id in starts {
            self.visit_statement(id, &mut result, visitor, registry);
        }
        result
    }

    fn visit_statement<V: NodeGraphVisitor<T>>(
        &self,
        id: NodeId<T>,
        result: &mut Vec<V::Output>,
        visitor: &mut V,
        registry: &Registry,
    ) {
        if let Some(node) = self.node(id) {
            let inputs = node
                .data
                .node_pins_in(registry)
                .into_iter()
                .filter(|pin| pin.is_parameter())
                .filter_map(|pin| {
                    self.node_neighbors_in(id, Some(pin.name()))
                        .next()
                        .map(|id| (pin.name().to_owned(), id))
                })
                .filter_map(|(name, id)| {
                    self.visit_expression(id, visitor, registry)
                        .map(|input| (name, input))
                })
                .collect();
            let pins_out = node.data.node_pins_out(registry);
            let scopes = pins_out
                .iter()
                .filter(|pin| pin.has_subscope())
                .filter_map(|pin| {
                    let id = self.node_neighbors_out(id, Some(pin.name())).next()?;
                    Some((id, pin.name().to_owned()))
                })
                .map(|(id, name)| {
                    let mut result = Vec::with_capacity(self.nodes.len());
                    self.visit_statement(id, &mut result, visitor, registry);
                    (name, result)
                })
                .collect();
            if visitor.visit_statement(node, inputs, scopes, result) {
                for pin in pins_out {
                    if pin.is_execute() && !pin.has_subscope() {
                        for id in self.node_neighbors_out(id, Some(pin.name())) {
                            self.visit_statement(id, result, visitor, registry);
                        }
                    }
                }
            }
        }
    }

    fn visit_expression<V: NodeGraphVisitor<T>>(
        &self,
        id: NodeId<T>,
        visitor: &mut V,
        registry: &Registry,
    ) -> Option<V::Input> {
        if let Some(node) = self.node(id) {
            let inputs = node
                .data
                .node_pins_in(registry)
                .into_iter()
                .filter(|pin| pin.is_parameter())
                .filter_map(|pin| {
                    self.node_neighbors_in(id, Some(pin.name()))
                        .next()
                        .map(|id| (pin.name().to_owned(), id))
                })
                .filter_map(|(name, id)| {
                    self.visit_expression(id, visitor, registry)
                        .map(|input| (name, input))
                })
                .collect();
            return visitor.visit_expression(node, inputs);
        }
        None
    }
}

impl<T: NodeDefinition + std::fmt::Debug> std::fmt::Debug for NodeGraph<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeGraph")
            .field("nodes", &self.nodes)
            .field("connections", &self.connections)
            .finish()
    }
}

pub trait NodeGraphVisitor<T: NodeDefinition> {
    type Input;
    type Output;

    fn visit_statement(
        &mut self,
        node: &Node<T>,
        inputs: HashMap<String, Self::Input>,
        scopes: HashMap<String, Vec<Self::Output>>,
        result: &mut Vec<Self::Output>,
    ) -> bool;

    fn visit_expression(
        &mut self,
        node: &Node<T>,
        inputs: HashMap<String, Self::Input>,
    ) -> Option<Self::Input>;
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use intuicio_core::prelude::*;
    use std::collections::HashMap;

    #[derive(Debug, Clone, PartialEq)]
    enum Script {
        Literal(i32),
        Return,
        Call(String),
        Scope(Vec<Script>),
    }

    impl NodeTypeInfo for String {
        fn struct_query(&self) -> StructQuery {
            StructQuery {
                name: Some(self.into()),
                ..Default::default()
            }
        }

        fn are_compatible(&self, other: &Self) -> bool {
            self == other
        }
    }

    #[derive(Debug, Clone)]
    enum Nodes {
        Start,
        Expression(i32),
        Result,
        Convert(String),
        Child,
    }

    impl NodeDefinition for Nodes {
        type TypeInfo = String;

        fn node_label(&self, _: &Registry) -> String {
            format!("{:?}", self)
        }

        fn node_pins_in(&self, _: &Registry) -> Vec<NodePin<Self::TypeInfo>> {
            match self {
                Nodes::Start => vec![],
                Nodes::Expression(_) => {
                    vec![NodePin::execute("In", false), NodePin::property("Value")]
                }
                Nodes::Result => vec![
                    NodePin::execute("In", false),
                    NodePin::parameter("Data", "i32".to_owned()),
                ],
                Nodes::Convert(_) => vec![
                    NodePin::execute("In", false),
                    NodePin::property("Name"),
                    NodePin::parameter("Data in", "i32".to_owned()),
                ],
                Nodes::Child => vec![NodePin::execute("In", false)],
            }
        }

        fn node_pins_out(&self, _: &Registry) -> Vec<NodePin<Self::TypeInfo>> {
            match self {
                Nodes::Start => vec![NodePin::execute("Out", false)],
                Nodes::Expression(_) => vec![
                    NodePin::execute("Out", false),
                    NodePin::parameter("Data", "i32".to_owned()),
                ],
                Nodes::Result => vec![],
                Nodes::Convert(_) => vec![
                    NodePin::execute("Out", false),
                    NodePin::parameter("Data out", "i32".to_owned()),
                ],
                Nodes::Child => vec![
                    NodePin::execute("Out", false),
                    NodePin::execute("Body", true),
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

        fn get_property(&self, property_name: &str) -> Option<PropertyValue> {
            match self {
                Nodes::Expression(value) => match property_name {
                    "Value" => PropertyValue::new(value).ok(),
                    _ => None,
                },
                Nodes::Convert(name) => match property_name {
                    "Name" => PropertyValue::new(name).ok(),
                    _ => None,
                },
                _ => None,
            }
        }

        fn set_property(&mut self, property_name: &str, property_value: PropertyValue) {
            #[allow(clippy::single_match)]
            match self {
                Nodes::Expression(value) => match property_name {
                    "Value" => {
                        if let Ok(v) = property_value.get_exact::<i32>() {
                            *value = v;
                        }
                    }
                    _ => {}
                },
                Nodes::Convert(name) => {
                    if let Ok(v) = property_value.get_exact::<String>() {
                        *name = v;
                    }
                }
                _ => {}
            }
        }
    }

    struct CompileNodesToScript;

    impl NodeGraphVisitor<Nodes> for CompileNodesToScript {
        type Input = ();
        type Output = Script;

        fn visit_statement(
            &mut self,
            node: &Node<Nodes>,
            _: HashMap<String, Self::Input>,
            mut scopes: HashMap<String, Vec<Self::Output>>,
            result: &mut Vec<Self::Output>,
        ) -> bool {
            match &node.data {
                Nodes::Result => result.push(Script::Return),
                Nodes::Convert(name) => result.push(Script::Call(name.to_owned())),
                Nodes::Child => {
                    if let Some(body) = scopes.remove("Body") {
                        result.push(Script::Scope(body));
                    }
                }
                Nodes::Expression(value) => result.push(Script::Literal(*value)),
                _ => {}
            }
            true
        }

        fn visit_expression(
            &mut self,
            _: &Node<Nodes>,
            _: HashMap<String, Self::Input>,
        ) -> Option<Self::Input> {
            None
        }
    }

    #[test]
    fn test_nodes() {
        let registry = Registry::default().with_basic_types();
        let mut graph = NodeGraph::default();
        let start = graph
            .add_node(Node::new(0, 0, Nodes::Start), &registry)
            .unwrap();
        let expression_child = graph
            .add_node(Node::new(0, 0, Nodes::Expression(42)), &registry)
            .unwrap();
        let convert_child = graph
            .add_node(Node::new(0, 0, Nodes::Convert("foo".to_owned())), &registry)
            .unwrap();
        let result_child = graph
            .add_node(Node::new(0, 0, Nodes::Result), &registry)
            .unwrap();
        let child = graph
            .add_node(Node::new(0, 0, Nodes::Child), &registry)
            .unwrap();
        let expression = graph
            .add_node(Node::new(0, 0, Nodes::Expression(42)), &registry)
            .unwrap();
        let convert = graph
            .add_node(Node::new(0, 0, Nodes::Convert("bar".to_owned())), &registry)
            .unwrap();
        let result = graph
            .add_node(Node::new(0, 0, Nodes::Result), &registry)
            .unwrap();
        graph.connect_nodes(NodeConnection::new(start, child, "Out", "In"));
        graph.connect_nodes(NodeConnection::new(child, expression_child, "Body", "In"));
        graph.connect_nodes(NodeConnection::new(
            expression_child,
            convert_child,
            "Out",
            "In",
        ));
        graph.connect_nodes(NodeConnection::new(
            expression_child,
            convert_child,
            "Data",
            "Data in",
        ));
        graph.connect_nodes(NodeConnection::new(
            convert_child,
            result_child,
            "Out",
            "In",
        ));
        graph.connect_nodes(NodeConnection::new(
            convert_child,
            result_child,
            "Data out",
            "Data",
        ));
        graph.connect_nodes(NodeConnection::new(child, expression, "Out", "In"));
        graph.connect_nodes(NodeConnection::new(expression, convert, "Out", "In"));
        graph.connect_nodes(NodeConnection::new(expression, convert, "Data", "Data in"));
        graph.connect_nodes(NodeConnection::new(convert, result, "Out", "In"));
        graph.connect_nodes(NodeConnection::new(convert, result, "Data out", "Data"));
        graph.validate(&registry).unwrap();
        assert_eq!(
            graph.visit(&mut CompileNodesToScript, &registry),
            vec![
                Script::Scope(vec![
                    Script::Literal(42),
                    Script::Call("foo".to_owned()),
                    Script::Return
                ]),
                Script::Literal(42),
                Script::Call("bar".to_owned()),
                Script::Return
            ]
        );
        assert_eq!(
            graph
                .node(expression)
                .unwrap()
                .data
                .get_property("Value")
                .unwrap(),
            PropertyValue::new(&42i32).unwrap(),
        );
        graph
            .node_mut(expression)
            .unwrap()
            .data
            .set_property("Value", PropertyValue::new(&10i32).unwrap());
        assert_eq!(
            graph
                .node(expression)
                .unwrap()
                .data
                .get_property("Value")
                .unwrap(),
            PropertyValue::new(&10i32).unwrap(),
        );
    }
}
