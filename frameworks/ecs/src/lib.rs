pub mod actor;
pub mod archetype;
pub mod bundle;
pub mod commands;
pub mod entity;
pub mod query;
pub mod world;

pub mod prelude {
    pub use crate::{commands::*, entity::*, world::*, Component};
}

pub trait Component: Send + Sync + 'static {}

impl<T: Send + Sync + 'static> Component for T {}
