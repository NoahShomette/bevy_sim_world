use bevy::{
    prelude::{
        Commands, Component, DespawnRecursiveExt, DetectChanges, Entity, Mut, Query,
        RemovedComponents, ResMut, Resource, With, World,
    },
    reflect::Reflect,
    utils::HashMap,
};
use serde::{Deserialize, Serialize};

use crate::{
    player::Player,
    saving::{SaveId, SimResourceId},
};

#[derive(Default, Clone, Eq, Debug, PartialEq, Component, Reflect, Serialize, Deserialize)]
pub struct SimChanged {
    pub players_seen: Vec<usize>,
}

impl SimChanged {
    /// Checks if all players that are marked as needs_state have been registered and returns the result
    pub fn all_seen(&self, players: &Vec<Player>) -> bool {
        for player in players.iter() {
            if player.needs_state && !self.players_seen.contains(&player.id()) {
                return false;
            }
        }
        true
    }

    /// Checks if the given player id has already been registered and returns the result. If the player
    /// id hasn't seen the changes then it marks it as seen and returns false. If the player id has seen
    /// the changes then it does nothing and returns true.
    pub fn check_and_register_seen(&mut self, id: usize) -> bool {
        return if self.players_seen.contains(&id) {
            true
        } else {
            self.players_seen.push(id);
            false
        };
    }

    /// Checks if the given player id has been registered and returns the results
    pub fn was_seen(&mut self, id: usize) -> bool {
        return self.players_seen.contains(&id);
    }
}

/// Resource inserted into the world that will be used to drive sending despawned object updates
#[derive(Clone, Eq, Debug, PartialEq, Resource, Reflect, Serialize, Deserialize)]
pub struct TrackedDespawns {
    pub despawned_objects: HashMap<Entity, SimChanged>,
}

/// Resource inserted into the world that will be used to drive sending resource changed updates
#[derive(Clone, Eq, Debug, PartialEq, Resource)]
pub struct ResourceChangeTracking {
    pub resources: HashMap<SimResourceId, SimChanged>,
}

/// Component inserted onto an entity that despawns it and includes that entity into [`TrackedDespawns`] resource
#[derive(Component)]
pub struct DespawnTracked;

/// System automatically inserted into the GameRunner::game_post_schedule to automatically handle despawning
/// entities and updating the DespawnedObjects resource
pub fn despawn_objects(
    mut commands: Commands,
    query: Query<Entity, With<DespawnTracked>>,
    mut despawns: ResMut<TrackedDespawns>,
) {
    for entity in query.iter() {
        despawns
            .despawned_objects
            .insert(entity, SimChanged::default());

        commands.entity(entity).despawn_recursive();
    }
}

/// For every entity containing the given component that has changed, inserts a Changed::default() component
pub fn track_component_changes<C: Component>(
    mut commands: Commands,
    query: Query<Entity, bevy::prelude::Changed<C>>,
    mut removed_components: RemovedComponents<C>,
) {
    for entity in query.iter() {
        commands.entity(entity).insert(SimChanged::default());
    }

    for entity in removed_components.read() {
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(SimChanged::default());
        }
    }
}

/// Checks if the given resource has changed and if so inserts its ComponentId into the
/// ResourceChangeTracking resource
pub fn track_resource_changes<R: Resource + SaveId>(world: &mut World) {
    if !world.contains_resource::<R>() {
        return;
    }
    world.resource_scope(|world, resource: Mut<R>| {
        if resource.is_changed() {
            world.resource_scope(|_world, mut resources: Mut<ResourceChangeTracking>| {
                resources
                    .resources
                    .insert(resource.save_id(), SimChanged::default());
            });
        }
    });
}

#[cfg(test)]
pub mod test {
    use bevy::{
        prelude::{Component, Mut, Resource, World},
        reflect::Reflect,
    };
    use serde::{Deserialize, Serialize};

    use crate::{
        game_builder::GameBuilder,
        requests::state_dif::StateDif,
        runner::{GameRuntime, TurnBasedGameRunner},
        saving::{SaveId, SimComponentId},
        SimWorld,
    };

    #[derive(Default, Component, Serialize, Deserialize, Reflect)]
    struct TestComponent(u32);

    impl SaveId for TestComponent {
        fn save_id(&self) -> SimComponentId {
            25
        }

        fn save_id_const() -> SimComponentId
        where
            Self: Sized,
        {
            25
        }

        #[doc = r" Serializes the state of the object at the given tick into binary. Only saves the keyframe and not the curve itself"]
        fn to_binary(&self) -> Option<Vec<u8>> {
            bincode::serialize(self).ok()
        }
    }

    // TODO: write tests for this
    #[test]
    fn test_component_change_tracking() {
        let mut world = World::new();
        let mut game = GameBuilder::<TurnBasedGameRunner>::new_game(TurnBasedGameRunner {
            turn_schedule: Default::default(),
        });
        game.register_component::<TestComponent>();
        game.build(&mut world);

        let mut game = world.remove_resource::<SimWorld>().unwrap();
        let mut game_runtime = world
            .remove_resource::<GameRuntime<TurnBasedGameRunner>>()
            .unwrap();

        let entity = game.world.spawn_empty().insert(TestComponent(0)).id();

        game_runtime.simulate(&mut game.world);

        let mut first_state = game.request(StateDif { for_player: 0 });

        let mut entity_mut = game.world.entity_mut(entity);
        let mut component = entity_mut.get_mut::<TestComponent>().unwrap();
        component.0 += 1;

        game_runtime.simulate(&mut game.world);

        let mut second_state = game.request(StateDif { for_player: 0 });

        let components = first_state.entities.pop().unwrap().components;

        let test_component_1 = components
            .iter()
            .find(|item| {
                if let Some(_) = bincode::deserialize::<TestComponent>(&item.component).ok() {
                    return true;
                }
                false
            })
            .unwrap();
        let Ok(test_component_1) =
            bincode::deserialize::<TestComponent>(&test_component_1.component)
        else {
            panic!("Couldn't find component")
        };

        let components = second_state.entities.pop().unwrap().components;

        let test_component_2 = components
            .iter()
            .find(|item| {
                if let Some(_) = bincode::deserialize::<TestComponent>(&item.component).ok() {
                    return true;
                }
                false
            })
            .unwrap();
        let Ok(test_component_2) =
            bincode::deserialize::<TestComponent>(&test_component_2.component)
        else {
            panic!("Couldn't find component")
        };

        assert_eq!(test_component_1.0, 0);
        assert_eq!(test_component_2.0, 1);
    }

    #[derive(Default, Resource, Reflect, Serialize, Deserialize)]
    struct TestResource(u32);

    impl SaveId for TestResource {
        fn save_id(&self) -> SimComponentId {
            25
        }

        fn save_id_const() -> SimComponentId
        where
            Self: Sized,
        {
            25
        }

        #[doc = r" Serializes the state of the object at the given tick into binary. Only saves the keyframe and not the curve itself"]
        fn to_binary(&self) -> Option<Vec<u8>> {
            bincode::serialize(self).ok()
        }
    }

    #[test]
    fn test_resource_change_tracking() {
        let mut world = World::new();
        let mut game = GameBuilder::<TurnBasedGameRunner>::new_game(TurnBasedGameRunner {
            turn_schedule: Default::default(),
        });
        game.register_resource::<TestResource>();
        game.build(&mut world);

        let mut game = world.remove_resource::<SimWorld>().unwrap();
        let mut game_runtime = world
            .remove_resource::<GameRuntime<TurnBasedGameRunner>>()
            .unwrap();

        game.world.insert_resource(TestResource(0));

        game_runtime.simulate(&mut game.world);

        let mut first_state = game.request(StateDif { for_player: 0 });

        game.world
            .resource_scope(|_, mut resource: Mut<TestResource>| {
                resource.0 += 1;
            });

        game_runtime.simulate(&mut game.world);

        let mut second_state = game.request(StateDif { for_player: 0 });

        let resource = first_state.resources.pop().unwrap();

        let Some(test_component_1) = bincode::deserialize::<TestResource>(&resource.resource).ok()
        else {
            panic!("Couldn't find component")
        };

        let resource = second_state.resources.pop().unwrap();

        let Ok(test_component_2) = bincode::deserialize::<TestResource>(&resource.resource) else {
            panic!("Couldn't find component")
        };

        assert_eq!(test_component_1.0, 0);
        assert_eq!(test_component_2.0, 1);
    }
}
