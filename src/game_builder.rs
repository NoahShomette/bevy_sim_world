use crate::change_detection::{despawn_objects, track_component_changes, track_resource_changes};
use crate::change_detection::{ResourceChangeTracking, TrackedDespawns};
use crate::command::{GameCommand, SimCommandMeta, SimCommandQueue, SimCommands};
use crate::player::{Player, PlayerList, PlayerMarker};
use crate::runner::{SimRuntime, PostBaseSets, PreBaseSets, SimRunner};
use crate::SimWorld;
use bevy::prelude::*;
use bevy_trait_query::RegisterExt;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::default::Default;

use crate::saving::{SimSerDeRegistry, SaveId};

/// GameBuilder that creates a new game and sets it up correctly
#[derive(Resource)]
pub struct SimBuilder<SR>
where
    SR: SimRunner + 'static,
{
    pub sim_runner: SR,
    /// A schedule that is run before the SimRunner::simulate function
    pub sim_pre_schedule: Schedule,
    /// A schedule that is run after the SimRunner::simulate function
    pub sim_post_schedule: Schedule,
    pub sim_world: World,
    /// A schedule that is run as the last item before inserting the Sim Resource during setup. Use
    /// this for systems that must be run once when the sim is setup and only then
    pub setup_schedule: Schedule,
    pub sim_serde_registry: SimSerDeRegistry,
    pub commands: Option<SimCommands>,
    pub next_player_id: usize,
    pub player_list: PlayerList,
}

impl<SR> SimBuilder<SR>
where
    SR: SimRunner,
{
    pub fn new_sim(sim_runner: SR) -> SimBuilder<SR> {
        let mut sim_world = World::new();

        sim_world.insert_resource(SimCommands::default());

        SimBuilder {
            sim_runner,
            sim_pre_schedule: SimBuilder::<SR>::default_sim_pre_schedule(),
            sim_post_schedule: SimBuilder::<SR>::default_sim_post_schedule(),
            sim_world,
            setup_schedule: SimBuilder::<SR>::default_setup_schedule(),
            sim_serde_registry: SimSerDeRegistry::default_registry(),
            commands: Default::default(),
            next_player_id: 0,
            player_list: PlayerList { players: vec![] },
        }
    }
    pub fn new_game_with_commands(
        commands: Vec<Box<dyn GameCommand>>,
        sim_runner: SR,
    ) -> SimBuilder<SR> {
        let mut sim_command_queue: Vec<SimCommandMeta> = vec![];

        for command in commands.into_iter() {
            let utc: DateTime<Utc> = Utc::now();
            sim_command_queue.push(SimCommandMeta {
                command,
                command_time: utc,
            })
        }

        let sim_world = World::new();

        SimBuilder {
            sim_runner,
            sim_pre_schedule: SimBuilder::<SR>::default_sim_pre_schedule(),
            sim_post_schedule: SimBuilder::<SR>::default_sim_post_schedule(),
            sim_world,
            setup_schedule: SimBuilder::<SR>::default_setup_schedule(),
            sim_serde_registry: SimSerDeRegistry::default_registry(),
            commands: Some(SimCommands {
                queue: SimCommandQueue {
                    queue: sim_command_queue,
                },
                history: Default::default(),
            }),
            next_player_id: 0,
            player_list: PlayerList { players: vec![] },
        }
    }

    /// Removes the [`SimCommands`] from the sim world and returns them. Make sure to reinsert the commands
    /// after using them
    pub fn remove_commands(&mut self) -> Option<SimCommands> {
        self.commands.take()
    }

    /// Inserts the given commands into the sim world
    pub fn insert_commands(&mut self, game_commands: SimCommands) {
        self.commands = Some(game_commands);
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
        self.sim_post_schedule
            .add_systems(track_component_changes::<C>.in_set(PostBaseSets::Main));
    }

    /// Registers a resource which will be tracked, updated, and reported in state events
    pub fn register_resource_track_changes<R>(&mut self)
    where
        R: Resource + SaveId,
    {
        self.sim_post_schedule
            .add_systems(track_resource_changes::<R>.in_set(PostBaseSets::Main));
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
        let schedule = Schedule::default();

        schedule
    }
    pub fn default_sim_pre_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule
            .configure_sets(
                (
                    PreBaseSets::Pre,
                    PreBaseSets::PreCommandFlush,
                    PreBaseSets::Main,
                    PreBaseSets::MainCommandFlush,
                    PreBaseSets::Post,
                    PreBaseSets::PostCommandFlush,
                )
                    .chain(),
            )
            .add_systems(apply_deferred.in_set(PreBaseSets::PreCommandFlush))
            .add_systems(apply_deferred.in_set(PreBaseSets::MainCommandFlush))
            .add_systems(apply_deferred.in_set(PreBaseSets::PostCommandFlush));

        schedule
    }

    pub fn default_sim_post_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule
            .configure_sets(
                (
                    PostBaseSets::PreCommandFlush,
                    PostBaseSets::Pre,
                    PostBaseSets::MainCommandFlush,
                    PostBaseSets::Main,
                    PostBaseSets::PostCommandFlush,
                    PostBaseSets::Post,
                )
                    .chain(),
            )
            .add_systems(apply_deferred.in_set(PostBaseSets::PreCommandFlush))
            .add_systems(apply_deferred.in_set(PostBaseSets::MainCommandFlush))
            .add_systems(apply_deferred.in_set(PostBaseSets::PostCommandFlush));

        schedule.add_systems(despawn_objects.in_set(PostBaseSets::Pre));
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
        self.setup_schedule.run(&mut self.sim_world);
        main_world.insert_resource::<SimRuntime<SR>>(SimRuntime {
            sim_runner: self.sim_runner,
            sim_pre_schedule: self.sim_pre_schedule,
            sim_post_schedule: self.sim_post_schedule,
        });
        self.sim_world
            .insert_resource(self.sim_serde_registry.clone());
        self.sim_world.insert_resource(TrackedDespawns {
            despawned_objects: Default::default(),
        });
        self.sim_world.insert_resource(ResourceChangeTracking {
            resources: Default::default(),
        });
        self.sim_world.insert_resource(self.player_list.clone());

        if let Some(commands) = self.commands.as_mut() {
            commands.execute_buffer(&mut self.sim_world);
        } else {
            self.commands = Some(SimCommands::default());
        }

        main_world.insert_resource(self.commands.unwrap());

        self.setup_schedule.run(&mut self.sim_world);

        main_world.insert_resource::<SimWorld>(SimWorld {
            world: self.sim_world,
            registry: self.sim_serde_registry,
            player_list: self.player_list,
        });
    }
}
