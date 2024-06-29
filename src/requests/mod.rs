use bevy::prelude::Entity;

use crate::{
    player::Player,
    saving::{ComponentBinaryState, SimResourceId},
    SimWorld,
};

pub mod all_state;
pub mod state_dif;

/// Trait used to make requests into the game world
pub trait SimRequest {
    type Output;
    fn request(&mut self, sim_world: &mut SimWorld) -> Self::Output;
}

/// Contains the state of a player, identified by a [`Player`] component
#[derive(Debug)]
pub struct PlayerState {
    pub player_id: Player,
    pub components: Vec<ComponentBinaryState>,
}

/// Contains the state of a [`Resource`]
#[derive(Debug)]
pub struct ResourceState {
    pub resource_id: SimResourceId,
    pub resource: Vec<u8>,
}

/// Contains an entities state, identified via its [`Entity`] component
#[derive(Debug)]
pub struct EntityState {
    pub entity: Entity,
    pub components: Vec<ComponentBinaryState>,
}

/// A list of state
#[derive(Debug, Default)]
pub struct SimState {
    pub players: Vec<PlayerState>,
    pub resources: Vec<ResourceState>,
    pub entities: Vec<EntityState>,
    pub despawned_objects: Vec<Entity>,
}
