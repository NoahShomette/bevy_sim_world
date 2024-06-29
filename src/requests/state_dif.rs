use bevy::prelude::{Entity, Mut, With, Without};

use crate::{
    change_detection::{DespawnTracked, ResourceChangeTracking, SimChanged, TrackedDespawns},
    player::Player,
    saving::{ComponentBinaryState, SaveId},
};

use super::{EntityState, PlayerState, SimRequest, SimState};

/// Returns only the state that has changed.
pub struct StateDif {
    pub for_player: usize,
}

impl SimRequest for StateDif {
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
            .query_filtered::<(&dyn SaveId, Entity, Option<&Player>, &mut SimChanged), (With<SimChanged>, Without<DespawnTracked>)>();

        for (saveable_components, entity, opt_player, mut changed) in
            query.iter_mut(&mut sim_world.world)
        {
            if changed.check_and_register_seen(self.for_player) {
                continue;
            }
            let mut components: Vec<ComponentBinaryState> = vec![];

            if let Some(player) = opt_player {
                for component in saveable_components.iter() {
                    if let Some((id, binary)) = component.save() {
                        components.push(ComponentBinaryState {
                            id,
                            component: binary,
                        });
                    }
                }

                state.players.push(PlayerState {
                    player_id: *player,
                    components,
                })
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
                    entity: entity,
                    components,
                })
            }
        }

        sim_world
            .world
            .resource_scope(|_, mut despawned_objects: Mut<TrackedDespawns>| {
                for (id, changed) in despawned_objects.despawned_objects.iter_mut() {
                    if !changed.check_and_register_seen(self.for_player) {
                        state.despawned_objects.push(*id);
                    }
                }
            });

        sim_world.world.resource_scope(
            |world, mut resource_change_tracking: Mut<ResourceChangeTracking>| {
                for (id, changed) in resource_change_tracking.resources.iter_mut() {
                    if !changed.check_and_register_seen(self.for_player) {
                        if let Some(resource_state) =
                            sim_world.registry.serialize_resource(id, &world)
                        {
                            state.resources.push(resource_state);
                        }
                    }
                }
            },
        );

        state
    }
}
