use crate::{
    archetype::{ArchetypeColumnInfo, ArchetypeError},
    world::{Relation, World, WorldError},
    Component,
};
use intuicio_core::{registry::Registry, types::TypeQuery};
use intuicio_data::type_hash::TypeHash;
use intuicio_framework_serde::{
    from_intermediate, to_intermediate, Intermediate, IntermediateResult, SerializationRegistry,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::error::Error;

#[derive(Debug)]
pub enum PrefabError {
    CouldNotFindType(TypeHash),
    CouldNotSerializeType {
        type_name: String,
        module_name: Option<String>,
    },
    CouldNotDeserializeType {
        type_name: String,
        module_name: Option<String>,
    },
    World(WorldError),
}

impl std::fmt::Display for PrefabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CouldNotFindType(type_hash) => {
                write!(f, "Could not find type by hash: {:?}", type_hash)
            }
            Self::CouldNotSerializeType {
                type_name,
                module_name,
            } => write!(
                f,
                "Could not serialize type: {}::{}",
                module_name.as_deref().unwrap_or_default(),
                type_name
            ),
            Self::CouldNotDeserializeType {
                type_name,
                module_name,
            } => write!(
                f,
                "Could not deserialize type: {}::{}",
                module_name.as_deref().unwrap_or_default(),
                type_name
            ),
            Self::World(error) => write!(f, "World error: {}", error),
        }
    }
}

impl From<WorldError> for PrefabError {
    fn from(value: WorldError) -> Self {
        Self::World(value)
    }
}

impl From<ArchetypeError> for PrefabError {
    fn from(value: ArchetypeError) -> Self {
        Self::World(WorldError::Archetype(value))
    }
}

impl Error for PrefabError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrefabArchetypeColumn {
    pub type_name: String,
    pub module_name: Option<String>,
    pub components: Vec<Intermediate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrefabArchetype {
    pub columns: Vec<PrefabArchetypeColumn>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prefab {
    pub archetypes: Vec<PrefabArchetype>,
}

impl Prefab {
    pub fn register_relation_serializer<T: Serialize + DeserializeOwned + Component>(
        serialization: &mut SerializationRegistry,
    ) {
        serialization.register::<Relation<T>>(
            |data| {
                Ok(Intermediate::Seq(
                    data.iter()
                        .map(|(item, entity)| {
                            Ok(Intermediate::Tuple(vec![
                                to_intermediate(item)?,
                                to_intermediate(&entity)?,
                            ]))
                        })
                        .collect::<IntermediateResult<Vec<_>>>()?,
                ))
            },
            |data, value| {
                let Intermediate::Seq(items) = value else {
                    return Err("Expected intermediate sequence".into());
                };
                for tuple in items {
                    let Intermediate::Tuple(tuple) = tuple else {
                        return Err("Expected intermediate tuple".into());
                    };
                    if tuple.len() != 2 {
                        return Err("Expected tuple to have 2 items".into());
                    }
                    data.add(from_intermediate(&tuple[0])?, from_intermediate(&tuple[1])?);
                }
                Ok(())
            },
        );
    }

    pub fn from_world<const LOCKING: bool>(
        world: &World,
        serialization: &SerializationRegistry,
        registry: &Registry,
    ) -> Result<Self, PrefabError> {
        let archetypes = world
            .archetypes()
            .map(|archetype| {
                let columns = archetype
                    .columns()
                    .map(|column| {
                        let type_ = registry
                            .find_type(TypeQuery {
                                type_hash: Some(column.type_hash()),
                                ..Default::default()
                            })
                            .ok_or_else(|| PrefabError::CouldNotFindType(column.type_hash()))?;
                        let components =
                            archetype.dynamic_column_iter::<LOCKING>(column.type_hash(), false)?;
                        let components = components
                            .map(|component| unsafe {
                                serialization
                                    .dynamic_serialize_from(column.type_hash(), component.data())
                                    .map_err(|_| PrefabError::CouldNotSerializeType {
                                        type_name: type_.type_name().to_owned(),
                                        module_name: type_
                                            .module_name()
                                            .map(|name| name.to_owned()),
                                    })
                            })
                            .collect::<Result<_, PrefabError>>()?;
                        Ok(PrefabArchetypeColumn {
                            type_name: type_.type_name().to_owned(),
                            module_name: type_.module_name().map(|name| name.to_owned()),
                            components,
                        })
                    })
                    .collect::<Result<_, PrefabError>>()?;
                Ok(PrefabArchetype { columns })
            })
            .collect::<Result<_, PrefabError>>()?;
        Ok(Self { archetypes })
    }

    // TODO: there is a problem with relations where those will be serialized as in
    // original version, so they will point to wrong entities after deserialization.
    // DON'T EXPOSE IT UNTIL YOU SOLVE THIS PROBLEM!
    // We could actually expand list of entities by traversing relations, that would
    // require to add a mechanism to gather entities graph from provided ones, along
    // with registering component traversal operations.
    // pub fn from_entities<const LOCKING: bool>(
    //     world: &World,
    //     entities: impl IntoIterator<Item = Entity>,
    //     serialization: &SerializationRegistry,
    //     registry: &Registry,
    // ) -> Result<Self, PrefabError> {
    //     let mut archetype_rows = HashMap::<u32, (&Archetype, Vec<usize>)>::new();
    //     for entity in entities {
    //         let id = world.entity_archetype_id(entity)?;
    //         if let Some((archetype, rows)) = archetype_rows.get_mut(&id) {
    //             rows.push(
    //                 archetype
    //                     .entities()
    //                     .index_of(entity)
    //                     .ok_or(WorldError::EntityDoesNotExists { entity })?,
    //             );
    //         } else {
    //             let archetype = world.archetype_by_id(id)?;
    //             archetype_rows.insert(
    //                 id,
    //                 (
    //                     archetype,
    //                     vec![archetype
    //                         .entities()
    //                         .index_of(entity)
    //                         .ok_or(WorldError::EntityDoesNotExists { entity })?],
    //                 ),
    //             );
    //         }
    //     }
    //     let archetypes = archetype_rows
    //         .into_values()
    //         .map(|(archetype, rows)| {
    //             let columns = archetype
    //                 .columns()
    //                 .map(|column| {
    //                     let type_ = registry
    //                         .find_type(TypeQuery {
    //                             type_hash: Some(column.type_hash()),
    //                             ..Default::default()
    //                         })
    //                         .ok_or_else(|| PrefabError::CouldNotFindType(column.type_hash()))?;
    //                     let components =
    //                         archetype.dynamic_column::<LOCKING>(column.type_hash(), false)?;
    //                     let components = rows
    //                         .iter()
    //                         .map(|row| unsafe {
    //                             serialization
    //                                 .dynamic_serialize_from(
    //                                     column.type_hash(),
    //                                     components.data(*row)?,
    //                                 )
    //                                 .map_err(|_| PrefabError::CouldNotSerializeType {
    //                                     type_name: type_.type_name().to_owned(),
    //                                     module_name: type_
    //                                         .module_name()
    //                                         .map(|name| name.to_owned()),
    //                                 })
    //                         })
    //                         .collect::<Result<_, PrefabError>>()?;
    //                     Ok(PrefabArchetypeColumn {
    //                         type_name: type_.type_name().to_owned(),
    //                         module_name: type_.module_name().map(|name| name.to_owned()),
    //                         components,
    //                     })
    //                 })
    //                 .collect::<Result<_, PrefabError>>()?;
    //             Ok(PrefabArchetype { columns })
    //         })
    //         .collect::<Result<_, PrefabError>>()?;
    //     Ok(Self { archetypes })
    // }

    pub fn to_world(
        &self,
        serialization: &SerializationRegistry,
        registry: &Registry,
    ) -> Result<World, PrefabError> {
        let mut world = World::default();
        for archetype in &self.archetypes {
            let column_types = archetype
                .columns
                .iter()
                .map(|column| {
                    registry
                        .find_type(TypeQuery {
                            name: Some(column.type_name.as_str().into()),
                            module_name: column
                                .module_name
                                .as_ref()
                                .map(|name| name.as_str().into()),
                            ..Default::default()
                        })
                        .ok_or_else(|| PrefabError::CouldNotDeserializeType {
                            type_name: column.type_name.to_owned(),
                            module_name: column.module_name.to_owned(),
                        })
                })
                .collect::<Result<Vec<_>, PrefabError>>()?;
            let column_info = column_types
                .iter()
                .map(|type_| ArchetypeColumnInfo::from_type(type_))
                .collect::<Vec<_>>();
            let rows_count = archetype
                .columns
                .iter()
                .map(|column| column.components.len())
                .min()
                .unwrap_or_default();
            for index in 0..rows_count {
                unsafe {
                    let (_, access) = world.spawn_uninitialized_raw(column_info.to_owned())?;
                    for ((column, info), type_) in archetype
                        .columns
                        .iter()
                        .zip(column_info.iter())
                        .zip(column_types.iter())
                    {
                        let data = access.data(info.type_hash())?;
                        let component = &column.components[index];
                        access.initialize_raw(type_)?;
                        serialization
                            .dynamic_deserialize_to(info.type_hash(), data, component)
                            .map_err(|_| PrefabError::CouldNotDeserializeType {
                                type_name: column.type_name.to_owned(),
                                module_name: column.module_name.to_owned(),
                            })?;
                    }
                }
            }
        }
        Ok(world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefab() {
        let mut registry = Registry::default().with_basic_types();
        Relation::<()>::install_to_registry(&mut registry);

        let mut serialization = SerializationRegistry::default().with_basic_types();
        Prefab::register_relation_serializer::<()>(&mut serialization);

        let mut world = World::default();
        let a = world.spawn((42usize,)).unwrap();
        let b = world.spawn((false, Relation::new((), a))).unwrap();
        let c = world.spawn((true, Relation::new((), b))).unwrap();

        {
            let prefab = Prefab::from_world::<true>(&world, &serialization, &registry).unwrap();
            let world2 = prefab.to_world(&serialization, &registry).unwrap();

            let mut entities = world.entities().collect::<Vec<_>>();
            let mut entities2 = world2.entities().collect::<Vec<_>>();
            entities.sort();
            entities2.sort();
            assert_eq!(entities, entities2);

            let old = world.component::<true, usize>(a).unwrap();
            let new = world2.component::<true, usize>(a).unwrap();
            assert_eq!(*old, *new);
            let old = world.component::<true, bool>(b).unwrap();
            let new = world2.component::<true, bool>(b).unwrap();
            assert_eq!(*old, *new);
            let old = world.component::<true, Relation<()>>(b).unwrap();
            let new = world2.component::<true, Relation<()>>(b).unwrap();
            assert_eq!(*old, *new);
            let old = world.component::<true, bool>(c).unwrap();
            let new = world2.component::<true, bool>(c).unwrap();
            assert_eq!(*old, *new);
            let old = world.component::<true, Relation<()>>(c).unwrap();
            let new = world2.component::<true, Relation<()>>(c).unwrap();
            assert_eq!(*old, *new);
        }

        // {
        //     let prefab =
        //         Prefab::from_entities::<true>(&world, [b, c], &serialization, &registry).unwrap();
        //     let world2 = prefab.to_world(&serialization, &registry).unwrap();

        //     let mut entities = world2.entities().collect::<Vec<_>>();
        //     entities.sort();
        //     let b = entities[0];
        //     let c = entities[1];

        //     let value = world2.component::<true, bool>(b).unwrap();
        //     assert!(!*value);
        //     let value = world2.component::<true, bool>(c).unwrap();
        //     assert!(*value);
        // }
    }
}
