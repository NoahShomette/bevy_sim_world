//! Any actions that affect the game world should be specified as a [`GameCommand`] and submitted to
//! through the [`GameCommands`] to enable saving, rollback, and more. A command should be entirely
//! self contained, everything needed to accurately recreate the command should be included. A command
//! **cannot** rely on any actions outside of it, only data. Eg, for MoveObject, you can't rely on
//! the moving object having an up to date [`CurrentMovementInformation`](crate::movement::CurrentMovementInformation)
//! component, you must calculate the move in the command
//!
//! To use in a system, request the [`GameCommands`] Resource, get the commands field, and call a defined
//! command or submit a custom command using commands.add().
//! ```rust
//! use bevy::prelude::{Bundle, Reflect, ResMut, World};
//! use bevy_ecs_tilemap::prelude::TilePos;
//! use bevy_ggf::game_core::command::{GameCommand, GameCommands};
//! use bevy_ggf::mapping::MapId;
//!
//! #[derive(Bundle, Default, + Reflect)]
//! pub struct CustomBundle{
//!     // Whatever components you want in your bundle - GameCommands::spawn_object will automatically
//!     // insert the GameId struct with the next id
//! }
//!     
//! fn spawn_object_built_in_command(
//!     // Request the GameCommands Resource - all actions in the game should be communicated through
//!     // this
//!     mut game_commands: ResMut<GameCommands>,
//! ){
//!     // Call whatever command on GameCommands - Add your own commands by writing an extension trait
//!     // and implementing that for GameCommands//!
//!
//!     game_commands.spawn_object(CustomBundle::default(), TilePos::new(1, 1), MapId{id: 0}, 0);
//! }
//!
//! // Create a struct for your custom command, use this to store whatever data you need to execute
//! // and rollback the commands
//! #[derive(Clone, Debug, Reflect)]
//! struct MyCustomCommand;
//!
//! // Impl GameCommand for your struct
//! impl GameCommand for MyCustomCommand{
//!     fn execute(&mut self, world: &mut World) -> Result<Option<Box<(dyn GameCommand + 'static)>>, String> {
//!         todo!() // Implement whatever your custom command should do here
//!     }
//!
//!     fn rollback(&mut self, world: &mut World) -> Result<Option<Box<(dyn GameCommand + 'static)>>, String> {
//!         todo!() // Implement how to reverse your custom command - you can use your struct to save
//!                 // any data you might need, like the GameId of an entity spawned, the transform
//!                 // that the entity was at before, etc
//!     }
//! }
//!
//! fn spawn_object_custom_command(
//!    mut game: ResMut<GameCommands>,
//! ){
//!     game.commands.add(MyCustomCommand);
//! }
//!
//! ```

use crate::SimWorld;
use bevy::log::info;
use bevy::prelude::{Mut, Reflect, Resource, World};
use chrono::{DateTime, Utc};

/// Executes all stored game commands by calling the command queue execute buffer function
pub fn execute_game_commands_buffer(world: &mut World) {
    world.resource_scope(|world, mut game_commands: Mut<GameCommands>| {
        world.resource_scope(|_world, mut game: Mut<SimWorld>| {
            game_commands.execute_buffer(&mut game.world);
        });
    });
}

/// Executes all rollbacks requested - panics if a rollback fails
pub fn execute_game_rollbacks_buffer(world: &mut World) {
    world.resource_scope(|world, mut game: Mut<GameCommands>| {
        while game.history.rollbacks != 0 {
            if let Some(mut command) = game.history.pop() {
                command.command.rollback(world).expect("Rollback failed");
                game.history.rolledback_history.push(command);
                info!("Rollbacked command");
            }
            game.history.rollbacks -= 1;
        }
    });
}

/// Executes all rollforwards requested - panics if an execute fails
pub fn execute_game_rollforward_buffer(world: &mut World) {
    world.resource_scope(|world, mut game: Mut<GameCommands>| {
        while game.history.rollforwards != 0 {
            if let Some(mut command) = game.history.rolledback_history.pop() {
                if let Ok(_) = command.command.execute(world) {
                    game.history.push(command.clone());
                } else {
                    info!("Rolledforward failed");
                }
            }
            game.history.rollforwards -= 1;
        }
    });
}

pub enum CommandType {
    System,
    Player,
}

#[derive(Clone)]
pub struct GameCommandMeta {
    pub command: Box<dyn GameCommand>,
    pub command_time: DateTime<Utc>,
    //command_type: CommandType,
}

/// A base trait defining an action that affects the game. Define your own to implement your own
/// custom commands that will be automatically saved, executed, and rolledback. The rollback function
/// **MUST** exactly roll the world back to as it was, excluding entity IDs.
/// ```rust
/// use bevy::prelude::World;
/// use bevy::reflect::Reflect;
/// use bevy_ggf::game_core::command::GameCommand;
/// #[derive(Clone, Debug, Reflect)]
///  struct MyCustomCommand;
///
///  impl GameCommand for MyCustomCommand{
///     fn execute(&mut self, world: &mut World) -> Result<(), String> {
///          todo!() // Implement whatever your custom command should do here
///      }
///
///     fn rollback(&mut self, world: &mut World) -> Result<(), String> {
///          todo!() // Implement how to reverse your custom command
///      }
///  }
///
/// ```
pub trait GameCommand: Send + GameCommandClone + Sync + Reflect + 'static {
    /// Execute the command
    fn execute(&mut self, world: &mut World) -> Result<(), String>;

    /// Command to rollback a given command. Must undo exactly what execute did to return the game state
    /// to exactly the same state as before the execute was done.
    ///
    /// NOTE: This has a default implementation that does nothing but return Ok. This is so that if you
    /// dont want to use rollback you aren't required to implement it for your commands. However if
    /// you **do** want to use it make sure you implement it correctly.
    //#[cfg(feature = "command_rollback")]
    fn rollback(&mut self, _world: &mut World) -> Result<(), String> {
        Ok(())
    }
}

/* TODO: Figure out if a closure is possible. Probably not since we have two functions, but either way
 it would be nice if we can but they can still do whatever they need otherwise
impl<F> GameCommand for F
    where
        F: FnOnce(&mut World) + Sync + Copy + Debug + GameCommandClone + Send + 'static,
{
    fn execute(self: &mut F, world: &mut World) -> Result<(), String> {
        Ok(self(world))
    }
    fn rollback(self: &mut F, world: &mut World) -> Result<(), String> {
        Ok(self(world))
    }
}

 */

impl Clone for Box<dyn GameCommand> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Helper trait to clone boxed Game Commands
pub trait GameCommandClone {
    fn clone_box(&self) -> Box<dyn GameCommand>;
}

impl<T> GameCommandClone for T
where
    T: 'static + GameCommand + Clone + ?Sized,
{
    fn clone_box(&self) -> Box<dyn GameCommand> {
        Box::new(self.clone())
    }
}

/// The queue of pending [`GameCommand`]s. Doesn't do anything until executed
#[derive(Default)]
pub struct GameCommandQueue {
    pub queue: Vec<GameCommandMeta>,
}

impl GameCommandQueue {
    /// Push a new command to the end of the queue
    pub fn push<C>(&mut self, command: C)
    where
        C: GameCommand,
    {
        let utc: DateTime<Utc> = Utc::now();
        let command_meta = GameCommandMeta {
            command: Box::from(command),
            command_time: utc,
        };
        self.queue.push(command_meta);
    }

    /// Take the last command in the queue. Returns None if queue is empty
    pub fn pop(&mut self) -> Option<GameCommandMeta> {
        self.queue.pop()
    }
}

/// The history of all commands sent for this [`Game`] instance - if a command rollback occurs the
/// command is discarded from the history. This means that the history contains only the commands
/// that led to this instance of the game
#[derive(Default)]
pub struct GameCommandsHistory {
    pub history: Vec<GameCommandMeta>,
    pub rolledback_history: Vec<GameCommandMeta>,
    rollbacks: u32,
    rollforwards: u32,
}

impl GameCommandsHistory {
    /// Push a command to the end of the history vec
    pub fn push(&mut self, command: GameCommandMeta) {
        self.history.push(command);
    }

    /// Take the last command in the queue. Returns None if queue is empty
    pub fn pop(&mut self) -> Option<GameCommandMeta> {
        self.history.pop()
    }

    /// Push a command to the end of the history vec
    pub fn push_rollback_history(&mut self, command: GameCommandMeta) {
        self.rolledback_history.push(command);
    }

    /// Take the last command in the queue. Returns None if queue is empty
    pub fn pop_rollback_history(&mut self) -> Option<GameCommandMeta> {
        self.rolledback_history.pop()
    }

    pub fn clear_rollback_history(&mut self) {
        self.rolledback_history.clear();
    }
}

/// A struct to hold, execute, and rollback [`GameCommand`]s. Use associated actions to access and
/// modify the game
#[derive(Default, Resource)]
pub struct GameCommands {
    pub queue: GameCommandQueue,
    pub history: GameCommandsHistory,
}

impl GameCommands {
    pub fn new() -> Self {
        GameCommands {
            queue: Default::default(),
            history: Default::default(),
        }
    }

    /// Drains the command buffer and attempts to execute each command. Will only push commands that
    /// succeed to the history. If commands dont succeed they are silently failed.
    pub fn execute_buffer(&mut self, world: &mut World) {
        for mut command in self.queue.queue.drain(..).into_iter() {
            match command.command.execute(world) {
                Ok(_) => {
                    self.history.push(command);
                }
                Err(error) => {
                    info!("execution failed with: {:?}", error);
                }
            }
            self.history.clear_rollback_history();
        }
    }

    /// Request a single rollback - The game will attempt to rollback the next time
    /// [`execute_game_rollbacks_buffer`] is called
    pub fn rollback_one(&mut self) {
        self.history.rollbacks += 1;
    }

    /// Request a specific number of rollbacks - The game will attempt these rollbacks the next time
    /// [`execute_game_rollbacks_buffer`] is called
    pub fn rollback_amount(&mut self, amount: u32) {
        self.history.rollbacks += amount;
    }

    pub fn rollforward(&mut self, amount: u32) {
        self.history.rollforwards += amount;
    }

    /// Add a custom command to the queue
    pub fn add<T>(&mut self, command: T) -> T
    where
        T: GameCommand + Clone,
    {
        self.queue.push(command.clone());
        command
    }
}
