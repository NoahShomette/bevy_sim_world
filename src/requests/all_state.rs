use bevy::prelude::{Entity, Mut, Without};

use crate::{
    change_detection::{DespawnTracked, ResourceChangeTracking, TrackedDespawns},
    player::Player,
    saving::{ComponentBinaryState, SaveId},
};

use super::{EntityState, PlayerState, SimRequest, SimState};

/// Returns all the state regardless of its changed status
pub struct AllState;

impl SimRequest for AllState {
    type Output = SimState;

    fn request(&mut self, sim_world: &mut crate::SimWorld) -> Self::Output {
        let mut state: SimState = SimState {
            players: vec![],
            resources: vec![],
            entities: vec![],
            despawned_objects: vec![],
        };

        let mut query = sim_world
            .world
            .query_filtered::<(&dyn SaveId, Entity, Option<&Player>), Without<DespawnTracked>>();

        for (saveable_components, entity, opt_player) in query.iter_mut(&mut sim_world.world) {
            let mut components: Vec<ComponentBinaryState> = vec![];
            if opt_player.is_some() {
                for component in saveable_components.iter() {
                    if let Some((id, binary)) = component.save() {
                        components.push(ComponentBinaryState {
                            id,
                            component: binary,
                        });
                    }
                }

                if let Some(player) = opt_player {
                    state.players.push(PlayerState {
                        components,
                        player_id: *player,
                    });
                }
            } else {
                for component in saveable_components.iter() {
                    if let Some((id, binary)) = component.save() {
                        components.push(ComponentBinaryState {
                            id,
                            component: binary,
                        });
                    }
                }
                state.entities.push(EntityState {
                    components,
                    entity: entity,
                });
            }
        }

        sim_world
            .world
            .resource_scope(|_, mut despawned_objects: Mut<TrackedDespawns>| {
                for (id, _) in despawned_objects.despawned_objects.iter_mut() {
                    state.despawned_objects.push(*id);
                }
            });
        sim_world.world.resource_scope(
            |world, mut resource_change_tracking: Mut<ResourceChangeTracking>| {
                for (id, _) in resource_change_tracking.resources.iter_mut() {
                    if let Some(resource_state) = sim_world.registry.serialize_resource(id, &world)
                    {
                        state.resources.push(resource_state);
                    }
                }
            },
        );

        state
    }
}
