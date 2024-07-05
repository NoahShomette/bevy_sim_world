use bevy::{
    ecs::{
        component::{Component, ComponentId},
        system::Resource,
        world::World,
    },
    prelude::EntityWorldMut,
    utils::HashMap,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::requests::ResourceState;

pub mod implements;

/// An id hand assigned to components using the [`SaveId`] trait that identifies each component
///
/// Is simply a u16 under the type
pub type SimComponentId = u16;

/// An id hand assigned to resources using the [`SaveId`] trait that identifies each component
///
/// Is simply a u16 under the type
pub type SimResourceId = u16;

#[derive(Debug)]
pub struct ComponentBinaryState {
    pub id: SimComponentId,
    pub component: Vec<u8>,
}

/// A registry that contains deserialization functions for game components
#[derive(Resource, Clone, Default)]
pub struct GameSerDeRegistry {
    pub component_de_map: HashMap<SimComponentId, ComponentDeserializeFn>,
    pub resource_de_map: HashMap<SimResourceId, ResourceDeserializeFn>,
    pub resource_se_map: HashMap<SimResourceId, ResourceSerializeFn>,
    pub resource_id_map: ResourceSaveComponentIdMap,
}

impl GameSerDeRegistry {
    pub fn new() -> GameSerDeRegistry {
        GameSerDeRegistry::default()
    }

    /// Registers a component into the [`GameSerDeRegistry`] for automatic serialization and deserialization
    pub fn register_component<C>(&mut self)
    where
        C: Component + Serialize + DeserializeOwned + SaveId,
    {
        if self.component_de_map.contains_key(&C::save_id_const()) {
            panic!(
                "SavingMap component_de_map already contains key {}",
                C::save_id_const(),
            )
        }
        self.component_de_map
            .insert(C::save_id_const(), component_deserialize_onto::<C>);
    }

    /// Registers a component into the [`GameSerDeRegistry`] for automatic serialization and deserialization
    pub fn register_resource<R>(&mut self)
    where
        R: Resource + Serialize + DeserializeOwned + SaveId,
    {
        if self.resource_de_map.contains_key(&R::save_id_const()) {
            panic!(
                "SavingMap component_de_map already contains key {}",
                R::save_id_const(),
            )
        }
        self.resource_de_map
            .insert(R::save_id_const(), resource_deserialize_into_world::<R>);
        self.resource_se_map
            .insert(R::save_id_const(), serialize_resource_from_world::<R>);
    }

    /// Deserializes the given component onto the given entity.
    pub fn deserialize_component_onto(
        &self,
        data: &ComponentBinaryState,
        entity: &mut EntityWorldMut,
    ) {
        if let Some(deserialize_fn) = self.component_de_map.get(&data.id) {
            deserialize_fn(&data.component, entity);
        }
    }

    /// Deserializes the given [`ResourceState`] into the given world.
    pub fn deserialize_resource(&self, resource_state: ResourceState, world: &mut World) {
        if let Some(deserialize_fn) = self.resource_de_map.get(&resource_state.resource_id) {
            deserialize_fn(&resource_state.resource, world);
        }
    }

    /// Serializes the given resource from the given world.
    pub fn serialize_resource(
        &self,
        resource_id: &SimResourceId,
        world: &World,
    ) -> Option<ResourceState> {
        if let Some(serialize_fn) = self.resource_se_map.get(resource_id) {
            serialize_fn(world)
        } else {
            None
        }
    }

    /// Adds the default registry which has all the basic Bevy_GGF components and resources
    pub fn default_registry() -> GameSerDeRegistry {
        let game_registry = GameSerDeRegistry::new();
        game_registry
    }
}

pub type ComponentDeserializeFn = fn(data: &Vec<u8>, entity: &mut EntityWorldMut);

/// Deserializes a binary component onto the given entity.
pub fn component_deserialize_onto<T>(data: &Vec<u8>, entity: &mut EntityWorldMut)
where
    T: Serialize + DeserializeOwned + Component + SaveId,
{
    let Some(keyframe) = bincode::deserialize::<T>(data).ok() else {
        return;
    };
    entity.insert(keyframe);
}

pub type ResourceDeserializeFn = fn(data: &Vec<u8>, world: &mut World);

pub type ResourceSerializeFn = fn(world: &World) -> Option<ResourceState>;

/// Deserializes a binary component onto the given entity.
pub fn resource_deserialize_into_world<T>(data: &Vec<u8>, world: &mut World)
where
    T: Serialize + DeserializeOwned + Resource + SaveId,
{
    let Some(resource) = bincode::deserialize::<T>(data).ok() else {
        return;
    };
    world.insert_resource(resource);
}

/// Deserializes a binary component onto the given entity.
pub fn serialize_resource_from_world<R>(world: &World) -> Option<ResourceState>
where
    R: Serialize + DeserializeOwned + Resource + SaveId,
{
    let Some(resource) = world.get_resource::<R>() else {
        return None;
    };
    let Some((id, binary)) = resource.save() else {
        return None;
    };

    Some(ResourceState {
        resource_id: id,
        resource: binary,
    })
}

#[derive(Clone, Default)]
pub struct ResourceSaveComponentIdMap {
    pub component_to_id: HashMap<ComponentId, SimResourceId>,
    pub id_to_component: HashMap<SimResourceId, ComponentId>,
}

impl ResourceSaveComponentIdMap {
    pub fn save_id(&self, resource_component_id: ComponentId) -> &SimResourceId {
        self.get_save_id(resource_component_id).unwrap()
    }
    pub fn get_save_id(&self, resource_component_id: ComponentId) -> Option<&SimResourceId> {
        self.component_to_id.get(&resource_component_id)
    }

    pub fn component_id(&self, sim_resource_id: SimResourceId) -> &ComponentId {
        self.get_component_id(sim_resource_id).unwrap()
    }
    pub fn get_component_id(&self, sim_resource_id: SimResourceId) -> Option<&ComponentId> {
        self.id_to_component.get(&sim_resource_id)
    }

    pub fn register_resource(
        &mut self,
        resource_component_id: ComponentId,
        sim_resource_id: SimResourceId,
    ) {
        self.id_to_component
            .insert(sim_resource_id, resource_component_id);
        self.component_to_id
            .insert(resource_component_id, sim_resource_id);
    }
}

/// Must be implemented on any components for objects that are expected to be saved
///
/// You must ensure that both this traits [save_id] function and [save_id_const] functions match
///
/// ## Example
/// ```
/// # use bevy_sim_world::saving::{SimComponentId, SaveId};
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Serialize, Deserialize)]
/// # struct UserComponent;
/// impl SaveId for UserComponent {
///     fn save_id(&self) -> SimComponentId {
///        9
///     }
///     
///     fn save_id_const() -> SimComponentId
///     where
///        Self: Sized,
///     {
///       9
///     }
///
///     fn to_binary(&self) -> Option<Vec<u8>> {
///       bincode::serialize(self).ok()
///     }   
/// }
///
/// ```
#[bevy_trait_query::queryable]
pub trait SaveId {
    fn save_id(&self) -> SimComponentId;
    fn save_id_const() -> SimComponentId
    where
        Self: Sized;

    /// Serializes the object into binary
    fn to_binary(&self) -> Option<Vec<u8>>;

    /// Saves self according to the implementation given in to_binary
    fn save(&self) -> Option<(SimComponentId, Vec<u8>)> {
        let Some(data) = self.to_binary() else {
            return None;
        };
        Some((self.save_id(), data))
    }
}
