use crate::change_detection::{despawn_objects, track_component_changes, track_resource_changes};
use crate::change_detection::{ResourceChangeTracking, TrackedDespawns};
use crate::command::{GameCommand, GameCommandMeta, GameCommandQueue, GameCommands};
use crate::player::{Player, PlayerList, PlayerMarker};
use crate::runner::{GameRunner, GameRuntime, PostBaseSets, PreBaseSets};
use crate::SimWorld;
use bevy::prelude::*;
use bevy_trait_query::RegisterExt;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::default::Default;

use crate::saving::{GameSerDeRegistry, SaveId};

/// GameBuilder that creates a new game and sets it up correctly
#[derive(Resource)]
pub struct GameBuilder<GR>
where
    GR: GameRunner + 'static,
{
    pub game_runner: GR,
    /// A schedule that is run before the GameRunner::simulate_game function
    pub game_pre_schedule: Schedule,
    /// A schedule that is run after the GameRunner::simulate_game function
    pub game_post_schedule: Schedule,
    pub game_world: World,
    /// A schedule that is run as the last item before inserting the Game Resource during setup. Use
    /// this for systems that must be run once when the game is setup and only then
    pub setup_schedule: Schedule,
    pub game_serde_registry: GameSerDeRegistry,
    pub commands: Option<GameCommands>,
    pub next_player_id: usize,
    pub player_list: PlayerList,
}

impl<GR> GameBuilder<GR>
where
    GR: GameRunner,
{
    pub fn new_game(game_runner: GR) -> GameBuilder<GR> {
        let mut game_world = World::new();

        game_world.insert_resource(GameCommands::default());

        GameBuilder {
            game_runner,
            game_pre_schedule: GameBuilder::<GR>::default_game_pre_schedule(),
            game_post_schedule: GameBuilder::<GR>::default_game_post_schedule(),
            game_world,
            setup_schedule: GameBuilder::<GR>::default_setup_schedule(),
            game_serde_registry: GameSerDeRegistry::default_registry(),
            commands: Default::default(),
            next_player_id: 0,
            player_list: PlayerList { players: vec![] },
        }
    }
    pub fn new_game_with_commands(
        commands: Vec<Box<dyn GameCommand>>,
        game_runner: GR,
    ) -> GameBuilder<GR> {
        let mut game_command_queue: Vec<GameCommandMeta> = vec![];

        for command in commands.into_iter() {
            let utc: DateTime<Utc> = Utc::now();
            game_command_queue.push(GameCommandMeta {
                command,
                command_time: utc,
            })
        }

        let game_world = World::new();

        GameBuilder {
            game_runner,
            game_pre_schedule: GameBuilder::<GR>::default_game_pre_schedule(),
            game_post_schedule: GameBuilder::<GR>::default_game_post_schedule(),
            game_world,
            setup_schedule: GameBuilder::<GR>::default_setup_schedule(),
            game_serde_registry: GameSerDeRegistry::default_registry(),
            commands: Some(GameCommands {
                queue: GameCommandQueue {
                    queue: game_command_queue,
                },
                history: Default::default(),
            }),
            next_player_id: 0,
            player_list: PlayerList { players: vec![] },
        }
    }

    /// Removes the [`GameCommands`] from the game world and returns them. Make sure to reinsert the commands
    /// after using them
    pub fn remove_commands(&mut self) -> Option<GameCommands> {
        self.commands.take()
    }

    /// Inserts the given commands into the game world
    pub fn insert_commands(&mut self, game_commands: GameCommands) {
        self.commands = Some(game_commands);
    }

    /// Adds the default registry which has all the basic Bevy_GGF components and resources
    pub fn add_default_registrations(&mut self) {
        self.game_world
            .register_component_as::<dyn SaveId, PlayerMarker>();
    }

    pub fn default_components_track_changes(&mut self) {
        self.register_component_track_changes::<Parent>();
        self.register_component_track_changes::<Children>();
        self.register_component_track_changes::<PlayerMarker>();
    }

    /// Inserts a system into GameRunner::game_post_schedule that will track the specified Component
    /// and insert a Changed::default() component when it detects a change
    pub fn register_component_track_changes<C>(&mut self)
    where
        C: Component,
    {
        self.game_post_schedule
            .add_systems(track_component_changes::<C>.in_set(PostBaseSets::Main));
    }

    /// Registers a resource which will be tracked, updated, and reported in state events
    pub fn register_resource_track_changes<R>(&mut self)
    where
        R: Resource + SaveId,
    {
        self.game_post_schedule
            .add_systems(track_resource_changes::<R>.in_set(PostBaseSets::Main));
    }

    /// Registers a component which will be tracked, updated, and reported in state events. Also adds
    /// the component to change detection
    pub fn register_component<Type>(&mut self)
    where
        Type: Component + SaveId + Serialize + DeserializeOwned,
    {
        self.game_serde_registry.register_component::<Type>();
        self.game_world.register_component_as::<dyn SaveId, Type>();
        self.register_component_track_changes::<Type>();
    }

    /// Registers a resource which will be tracked, updated, and reported in state events. Also adds
    /// the resource to change detection
    pub fn register_resource<Type>(&mut self)
    where
        Type: Resource + SaveId + Serialize + DeserializeOwned,
    {
        self.game_serde_registry.register_resource::<Type>();
        self.register_resource_track_changes::<Type>();
    }

    pub fn default_setup_schedule() -> Schedule {
        let schedule = Schedule::default();

        schedule
    }
    pub fn default_game_pre_schedule() -> Schedule {
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

    pub fn default_game_post_schedule() -> Schedule {
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
            .game_world
            .spawn(Player::new(new_player_id, needs_state));
        self.player_list
            .players
            .push(Player::new(new_player_id, needs_state));
        (new_player_id, player_entity)
    }

    pub fn build(mut self, main_world: &mut World) {
        self.setup_schedule.run(&mut self.game_world);
        main_world.insert_resource::<GameRuntime<GR>>(GameRuntime {
            game_runner: self.game_runner,
            game_pre_schedule: self.game_pre_schedule,
            game_post_schedule: self.game_post_schedule,
        });
        self.game_world
            .insert_resource(self.game_serde_registry.clone());
        self.game_world.insert_resource(TrackedDespawns {
            despawned_objects: Default::default(),
        });
        self.game_world.insert_resource(ResourceChangeTracking {
            resources: Default::default(),
        });
        self.game_world.insert_resource(self.player_list.clone());

        if let Some(commands) = self.commands.as_mut() {
            commands.execute_buffer(&mut self.game_world);
        } else {
            self.commands = Some(GameCommands::default());
        }

        main_world.insert_resource(self.commands.unwrap());

        self.setup_schedule.run(&mut self.game_world);

        main_world.insert_resource::<SimWorld>(SimWorld {
            world: self.game_world,
            registry: self.game_serde_registry,
            player_list: self.player_list,
        });
    }
}
