use crate::change_detection::{track_component_changes, track_resource_changes, TrackingSet};
use crate::change_detection::{ResourceChangeTracking, TrackedDespawns};
use crate::player::{Player, PlayerList, PlayerMarker};
use crate::SimWorld;
use bevy::prelude::*;
use bevy_trait_query::RegisterExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::default::Default;

use crate::saving::{SaveId, SimSerDeRegistry};

/// GameBuilder that creates a new game and sets it up correctly
#[derive(Resource)]
pub struct SimBuilder {
    pub sim_world: World,
    /// A schedule that contains resource and component state tracking systems
    pub tracking_schedule: Schedule,
    pub sim_serde_registry: SimSerDeRegistry,
    pub next_player_id: usize,
    pub player_list: PlayerList,
}

impl SimBuilder {
    pub fn new_sim() -> SimBuilder {
        let sim_world = World::new();

        SimBuilder {
            sim_world,
            tracking_schedule: SimBuilder::default_setup_schedule(),
            sim_serde_registry: SimSerDeRegistry::default_registry(),
            next_player_id: 0,
            player_list: PlayerList { players: vec![] },
        }
    }

    /// Adds the default registry which has all the basic Bevy_GGF components and resources
    pub fn add_default_registrations(&mut self) {
        self.sim_world
            .register_component_as::<dyn SaveId, PlayerMarker>();
    }

    pub fn default_components_track_changes(&mut self) {
        self.register_component_track_changes::<Parent>();
        self.register_component_track_changes::<Children>();
        self.register_component_track_changes::<PlayerMarker>();
    }

    /// Inserts a system into SimRunner::simpost_schedule that will track the specified Component
    /// and insert a Changed::default() component when it detects a change
    pub fn register_component_track_changes<C>(&mut self)
    where
        C: Component,
    {
        self.tracking_schedule
            .add_systems(track_component_changes::<C>.in_set(TrackingSet::Component));
    }

    /// Registers a resource which will be tracked, updated, and reported in state events
    pub fn register_resource_track_changes<R>(&mut self)
    where
        R: Resource + SaveId,
    {
        self.tracking_schedule
            .add_systems(track_resource_changes::<R>.in_set(TrackingSet::Resource));
    }

    /// Registers a component which will be tracked, updated, and reported in state events. Also adds
    /// the component to change detection
    pub fn register_component<Type>(&mut self)
    where
        Type: Component + SaveId + Serialize + DeserializeOwned,
    {
        self.sim_serde_registry.register_component::<Type>();
        self.sim_world.register_component_as::<dyn SaveId, Type>();
        self.register_component_track_changes::<Type>();
    }

    /// Registers a resource which will be tracked, updated, and reported in state events. Also adds
    /// the resource to change detection
    pub fn register_resource<Type>(&mut self)
    where
        Type: Resource + SaveId + Serialize + DeserializeOwned,
    {
        self.sim_serde_registry.register_resource::<Type>();
        self.register_resource_track_changes::<Type>();
    }

    pub fn default_setup_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.configure_sets((TrackingSet::Component, TrackingSet::Resource).chain());
        schedule
    }

    pub fn add_player(&mut self, needs_state: bool) -> (usize, EntityWorldMut) {
        let new_player_id = self.next_player_id;
        self.next_player_id += 1;
        let player_entity = self
            .sim_world
            .spawn(Player::new(new_player_id, needs_state));
        self.player_list
            .players
            .push(Player::new(new_player_id, needs_state));
        (new_player_id, player_entity)
    }

    pub fn build(mut self, main_world: &mut World) {
        self.sim_world
            .insert_resource(self.sim_serde_registry.clone());
        self.sim_world.insert_resource(TrackedDespawns {
            despawned_objects: Default::default(),
        });
        self.sim_world.insert_resource(ResourceChangeTracking {
            resources: Default::default(),
        });
        self.sim_world.insert_resource(self.player_list.clone());

        main_world.insert_resource::<SimWorld>(SimWorld {
            world: self.sim_world,
            registry: self.sim_serde_registry,
            player_list: self.player_list,
            tracking_schedule: self.tracking_schedule,
        });
    }
}
