use crate::{
    archetype::{ArchetypeColumnInfo, ArchetypeEntityRowAccess},
    Component,
};
use intuicio_core::object::{DynamicObject, TypedDynamicObject};
use intuicio_data::{managed::DynamicManaged, type_hash::TypeHash};
use std::alloc::dealloc;

#[derive(Default)]
pub struct DynamicBundle {
    components: Vec<DynamicManaged>,
}

impl DynamicBundle {
    pub fn with_component<T>(mut self, data: T) -> Result<Self, T> {
        self.add_component(data)?;
        Ok(self)
    }

    pub fn with_component_raw(mut self, data: DynamicManaged) -> Self {
        self.add_component_raw(data);
        self
    }

    pub fn add_component<T>(&mut self, data: T) -> Result<(), T> {
        self.add_component_raw(DynamicManaged::new(data)?);
        Ok(())
    }

    pub fn add_component_raw(&mut self, data: DynamicManaged) {
        if let Some(index) = self
            .components
            .iter()
            .position(|component| component.type_hash() == data.type_hash())
        {
            self.components[index] = data;
        } else {
            self.components.push(data);
        }
    }

    pub fn remove_component<T>(&mut self) -> Option<T> {
        self.remove_component_raw(TypeHash::of::<T>())?
            .consume()
            .ok()
    }

    pub fn remove_component_raw(&mut self, type_hash: TypeHash) -> Option<DynamicManaged> {
        if let Some(index) = self
            .components
            .iter()
            .position(|component| component.type_hash() == &type_hash)
        {
            Some(self.components.swap_remove(index))
        } else {
            None
        }
    }
}

pub trait BundleColumns {
    fn columns_static() -> Vec<ArchetypeColumnInfo>;

    fn columns(&self) -> Vec<ArchetypeColumnInfo> {
        Self::columns_static()
    }
}

impl BundleColumns for () {
    fn columns_static() -> Vec<ArchetypeColumnInfo> {
        vec![]
    }
}

impl BundleColumns for Vec<ArchetypeColumnInfo> {
    fn columns_static() -> Vec<ArchetypeColumnInfo> {
        vec![]
    }

    fn columns(&self) -> Vec<ArchetypeColumnInfo> {
        self.to_owned()
    }
}

impl BundleColumns for DynamicObject {
    fn columns_static() -> Vec<ArchetypeColumnInfo> {
        vec![]
    }

    fn columns(&self) -> Vec<ArchetypeColumnInfo> {
        self.property_values()
            .map(|object| {
                let handle = object.type_handle();
                unsafe {
                    ArchetypeColumnInfo::new_raw(
                        handle.type_hash(),
                        *handle.layout(),
                        handle.finalizer(),
                    )
                }
            })
            .collect()
    }
}

impl BundleColumns for TypedDynamicObject {
    fn columns_static() -> Vec<ArchetypeColumnInfo> {
        vec![]
    }

    fn columns(&self) -> Vec<ArchetypeColumnInfo> {
        self.property_values()
            .map(|object| {
                let handle = object.type_handle();
                unsafe {
                    ArchetypeColumnInfo::new_raw(
                        handle.type_hash(),
                        *handle.layout(),
                        handle.finalizer(),
                    )
                }
            })
            .collect()
    }
}

impl BundleColumns for DynamicBundle {
    fn columns_static() -> Vec<ArchetypeColumnInfo> {
        vec![]
    }

    fn columns(&self) -> Vec<ArchetypeColumnInfo> {
        self.components
            .iter()
            .map(|component| unsafe {
                ArchetypeColumnInfo::new_raw(
                    *component.type_hash(),
                    *component.layout(),
                    component.finalizer(),
                )
            })
            .collect()
    }
}

macro_rules! impl_bundle_columns_tuple {
    ($($type:ident),+) => {
        impl<$($type: Component),+> BundleColumns for ($($type,)+) {
            fn columns_static() -> Vec<ArchetypeColumnInfo> {
                vec![$(ArchetypeColumnInfo::new::<$type>()),+]
            }
        }
    };
}

impl_bundle_columns_tuple!(A);
impl_bundle_columns_tuple!(A, B);
impl_bundle_columns_tuple!(A, B, C);
impl_bundle_columns_tuple!(A, B, C, D);
impl_bundle_columns_tuple!(A, B, C, D, E);
impl_bundle_columns_tuple!(A, B, C, D, E, F);
impl_bundle_columns_tuple!(A, B, C, D, E, F, G);
impl_bundle_columns_tuple!(A, B, C, D, E, F, G, H);
impl_bundle_columns_tuple!(A, B, C, D, E, F, G, H, I);
impl_bundle_columns_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_bundle_columns_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_bundle_columns_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_bundle_columns_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_bundle_columns_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_bundle_columns_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_bundle_columns_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

pub trait Bundle: BundleColumns {
    fn initialize_into(self, access: &ArchetypeEntityRowAccess);
}

impl Bundle for () {
    fn initialize_into(self, _: &ArchetypeEntityRowAccess) {}
}

impl Bundle for DynamicObject {
    fn initialize_into(mut self, access: &ArchetypeEntityRowAccess) {
        for (_, object) in self.drain() {
            unsafe {
                let (handle, source_memory) = object.into_inner();
                let target_memory = access.data(handle.type_hash()).unwrap();
                target_memory.copy_from(source_memory, handle.layout().size());
                dealloc(source_memory, *handle.layout());
            }
        }
    }
}

impl Bundle for TypedDynamicObject {
    fn initialize_into(mut self, access: &ArchetypeEntityRowAccess) {
        for (_, object) in self.drain() {
            unsafe {
                let (handle, source_memory) = object.into_inner();
                let target_memory = access.data(handle.type_hash()).unwrap();
                target_memory.copy_from(source_memory, handle.layout().size());
                dealloc(source_memory, *handle.layout());
            }
        }
    }
}

impl Bundle for DynamicBundle {
    fn initialize_into(self, access: &ArchetypeEntityRowAccess) {
        for component in self.components {
            unsafe {
                let (type_hash, _, source_memory, layout, _) = component.into_inner();
                let target_memory = access.data(type_hash).unwrap();
                target_memory.copy_from(source_memory, layout.size());
                dealloc(source_memory, layout);
            }
        }
    }
}

macro_rules! impl_bundle_tuple {
    ($($type:ident),+) => {
        impl<$($type: Component),+> Bundle for ($($type,)+) {
            fn initialize_into(self, access: &ArchetypeEntityRowAccess) {
                #[allow(non_snake_case)]
                let ($($type,)+) = self;
                $(
                    unsafe { access.initialize($type).unwrap(); };
                )+
            }
        }
    };
}

impl_bundle_tuple!(A);
impl_bundle_tuple!(A, B);
impl_bundle_tuple!(A, B, C);
impl_bundle_tuple!(A, B, C, D);
impl_bundle_tuple!(A, B, C, D, E);
impl_bundle_tuple!(A, B, C, D, E, F);
impl_bundle_tuple!(A, B, C, D, E, F, G);
impl_bundle_tuple!(A, B, C, D, E, F, G, H);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

pub struct BundleChain<A: Bundle, B: Bundle>(pub A, pub B);

impl<A: Bundle, B: Bundle> BundleColumns for BundleChain<A, B> {
    fn columns_static() -> Vec<ArchetypeColumnInfo> {
        let mut result = A::columns_static();
        result.extend(B::columns_static());
        result
    }
}

impl<A: Bundle, B: Bundle> Bundle for BundleChain<A, B> {
    fn initialize_into(self, access: &ArchetypeEntityRowAccess) {
        self.0.initialize_into(access);
        self.1.initialize_into(access);
    }
}
