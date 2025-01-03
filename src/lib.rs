//!

use crate::change_detection::SimChanged;
use crate::player::PlayerList;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use bevy_trait_query::RegisterExt;
use change_detection::{
    track_component_changes, track_resource_changes, ResourceChangeTracking, TrackedDespawns,
    TrackingSet,
};
use player::{Player, PlayerMarker};
use requests::SimRequest;
use saving::{SaveId, SimResourceId};
use serde::{de::DeserializeOwned, Serialize};

use self::saving::SimSerDeRegistry;

pub mod change_detection;
pub mod player;
pub mod requests;
pub mod saving;

/// A separate world used to separate simulations
#[derive(Resource, Component)]
pub struct SimWorld {
    /// A bevy world
    pub world: World,
    /// Holds component and resource registrations
    pub registry: SimSerDeRegistry,
    /// List of all players in the sim. Used with state and changed
    pub player_list: PlayerList,
    /// Schedule containing systems to track component and resource state
    pub tracking_schedule: Schedule,
}

impl SimWorld {
    pub fn new() -> SimWorld {
        let mut sim_world = World::new();
        sim_world.insert_resource(SimSerDeRegistry::default_registry());
        sim_world.insert_resource(TrackedDespawns {
            despawned_objects: Default::default(),
        });
        sim_world.insert_resource(ResourceChangeTracking {
            resources: Default::default(),
        });
        sim_world.insert_resource(PlayerList {
            players: vec![],
            next_player_id: 0,
        });
        SimWorld {
            world: sim_world,
            tracking_schedule: SimWorld::default_setup_schedule(),
            registry: SimSerDeRegistry::default_registry(),
            player_list: PlayerList {
                players: vec![],
                next_player_id: 0,
            },
        }
    }

    /// Makes a request to the sim world and returns the results
    pub fn request<Request: SimRequest>(&mut self, mut request: Request) -> Request::Output {
        request.request(self)
    }

    /// Simple function that will clear all changed components that have been fully seen as well as
    /// the [`TrackedDespawns`] (it despawns marked entities) resource and the [`ResourceChangeTracking`] resource.
    pub fn clear_changed(&mut self, player_list: &PlayerList) {
        let mut system_state: SystemState<(Query<(Entity, &SimChanged)>, Commands)> =
            SystemState::new(&mut self.world);
        let (changed_query, mut commands) = system_state.get(&self.world);
        for (entity, changed) in changed_query.iter() {
            if changed.all_seen(&player_list.players) {
                commands.entity(entity).remove::<SimChanged>();
            }
        }

        self.world
            .resource_scope(|_world, mut despawned_objects: Mut<TrackedDespawns>| {
                let mut index_to_remove: Vec<Entity> = vec![];
                for (id, changed) in despawned_objects.despawned_objects.iter_mut() {
                    if changed.all_seen(&player_list.players) {
                        index_to_remove.push(*id);
                    }
                }
                for id in index_to_remove {
                    despawned_objects.despawned_objects.remove(&id);
                }
            });

        self.world.resource_scope(
            |_world, mut resource_change_tracking: Mut<ResourceChangeTracking>| {
                let mut index_to_remove: Vec<SimResourceId> = vec![];
                for (id, changed) in resource_change_tracking.resources.iter_mut() {
                    if changed.all_seen(&player_list.players) {
                        index_to_remove.push(*id);
                    }
                }
                for id in index_to_remove {
                    resource_change_tracking.resources.remove(&id);
                }
            },
        );

        system_state.apply(&mut self.world);
    }
}

impl SimWorld {
    /// Adds the default registry which has all the basic Bevy_GGF components and resources
    pub fn add_default_registrations(&mut self) {
        self.world
            .register_component_as::<dyn SaveId, PlayerMarker>();
        self.world.register_component_as::<dyn SaveId, Player>();
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
        self.registry.register_component::<Type>();
        self.world.register_component_as::<dyn SaveId, Type>();
        self.register_component_track_changes::<Type>();
        self.world.insert_resource(self.registry.clone());
    }

    /// Registers a resource which will be tracked, updated, and reported in state events. Also adds
    /// the resource to change detection
    pub fn register_resource<Type>(&mut self)
    where
        Type: Resource + SaveId + Serialize + DeserializeOwned,
    {
        self.registry.register_resource::<Type>();
        self.register_resource_track_changes::<Type>();
        self.world.insert_resource(self.registry.clone());
    }

    pub fn default_setup_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.configure_sets((TrackingSet::Component, TrackingSet::Resource).chain());
        schedule
    }

    pub fn add_player(&mut self, needs_state: bool) -> (usize, EntityWorldMut) {
        let new_player_id = self.player_list.next_player_id;
        self.player_list.next_player_id += 1;
        self.player_list
            .players
            .push(Player::new(new_player_id, needs_state));
        self.world.insert_resource(self.player_list.clone());
        let player_entity = self.world.spawn(Player::new(new_player_id, needs_state));
        (new_player_id, player_entity)
    }
}
