use intuicio_core::{registry::Registry, IntuicioStruct};
use intuicio_derive::*;
use serde::{Deserialize, Serialize};

/// Entity ids start with 1, 0 is considered invalid.
#[derive(
    IntuicioStruct, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[intuicio(module_name = "ecs_entity")]
pub struct Entity {
    #[intuicio(ignore)]
    pub(crate) id: u32,
    #[intuicio(ignore)]
    pub(crate) generation: u32,
}

impl Default for Entity {
    fn default() -> Self {
        Self::INVALID
    }
}

#[intuicio_methods(module_name = "ecs_entity")]
impl Entity {
    pub const INVALID: Self = unsafe { Self::new_unchecked(u32::MAX, 0) };

    pub const fn new(id: u32, generation: u32) -> Option<Self> {
        if id < u32::MAX {
            Some(Self { id, generation })
        } else {
            None
        }
    }

    /// # Safety
    pub const unsafe fn new_unchecked(id: u32, generation: u32) -> Self {
        Self { id, generation }
    }

    #[intuicio_method()]
    pub const fn is_valid(self) -> bool {
        self.id < u32::MAX
    }

    #[intuicio_method()]
    pub const fn id(self) -> u32 {
        self.id
    }

    #[intuicio_method()]
    pub const fn generation(self) -> u32 {
        self.generation
    }

    #[intuicio_method()]
    pub const fn to_u64(self) -> u64 {
        ((self.generation as u64) << 32) | self.id as u64
    }

    #[intuicio_method()]
    pub const fn from_u64(value: u64) -> Self {
        Self {
            generation: (value >> 32) as u32,
            id: value as u32,
        }
    }

    pub(crate) const fn bump_generation(mut self) -> Self {
        self.generation = self.generation.wrapping_add(1);
        self
    }
}

impl std::fmt::Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}:#{}", self.id, self.generation)
    }
}

impl Entity {
    pub fn install(registry: &mut Registry) {
        registry.add_type(Self::define_struct(registry));
        registry.add_function(Self::is_valid__define_function(registry));
        registry.add_function(Self::id__define_function(registry));
        registry.add_function(Self::generation__define_function(registry));
        registry.add_function(Self::to_u64__define_function(registry));
        registry.add_function(Self::from_u64__define_function(registry));
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EntityDenseMap {
    inner: Vec<Entity>,
}

impl EntityDenseMap {
    pub fn with_capacity(mut capacity: usize) -> Self {
        capacity = capacity.next_power_of_two().max(1);
        Self {
            inner: Vec::with_capacity(capacity),
        }
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn insert(&mut self, entity: Entity) -> Result<usize, usize> {
        if let Some(index) = self.index_of(entity) {
            Err(index)
        } else {
            if self.inner.len() == self.inner.capacity() {
                self.inner.reserve_exact(self.inner.capacity());
            }
            let index = self.inner.len();
            self.inner.push(entity);
            Ok(index)
        }
    }

    pub fn remove(&mut self, entity: Entity) -> Option<usize> {
        let index = self.index_of(entity)?;
        self.inner.swap_remove(index);
        if self.inner.len() == self.inner.capacity() / 2 {
            self.inner.shrink_to_fit();
        }
        Some(index)
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.inner.contains(&entity)
    }

    pub fn index_of(&self, entity: Entity) -> Option<usize> {
        self.inner.iter().position(|e| *e == entity)
    }

    pub fn get(&self, index: usize) -> Option<Entity> {
        self.inner.get(index).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.inner.iter().copied()
    }
}
