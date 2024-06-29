//!

use crate::change_detection::SimChanged;
use crate::player::PlayerList;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use change_detection::{ResourceChangeTracking, TrackedDespawns};
use requests::SimRequest;
use saving::SimResourceId;

use self::saving::GameSerDeRegistry;

pub mod change_detection;
pub mod command;
pub mod game_builder;
pub mod player;
pub mod requests;
pub mod runner;
pub mod saving;

/// Holds all the actual game information
#[derive(Resource)]
pub struct SimWorld {
    /// A world that should hold all sim state
    pub world: World,
    /// Holds component and resource registrations that will be diffed and updated
    pub registry: GameSerDeRegistry,
    /// List of all players in the sim. Used with state and changed
    pub player_list: PlayerList,
}

impl SimWorld {
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

    pub fn execute_game_commands(&mut self) {}
}
