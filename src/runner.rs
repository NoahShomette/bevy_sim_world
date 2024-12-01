use bevy::prelude::{Resource, Schedule, SystemSet, World};

/// Runtime that is used to drive the game. Users can implement whatever the want onto the GameRunner
/// and then call [GameRuntime::simulate()] in order to drive their game forward.
#[derive(Resource)]
pub struct SimRuntime<T>
where
    T: SimRunner,
{
    pub sim_runner: T,
    pub sim_pre_schedule: Schedule,
    pub sim_post_schedule: Schedule,
}

impl<T> SimRuntime<T>
where
    T: SimRunner,
{
    pub fn simulate(&mut self, mut world: &mut World) {
        self.sim_pre_schedule.run(&mut world);
        self.sim_runner.simulate(&mut world);
        self.sim_post_schedule.run(&mut world);
    }
}

// SystemSet for the GameRunner FrameworkPostSchedule
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum PostBaseSets {
    PreCommandFlush,
    Pre,
    MainCommandFlush,
    Main,
    PostCommandFlush,
    Post,
}

// SystemSet for the GameRunner FrameworkPreSchedule
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum PreBaseSets {
    Pre,
    PreCommandFlush,
    Main,
    MainCommandFlush,
    Post,
    PostCommandFlush,
}

/// The [`GameRunner`] represents the actual *game* logic that you want run whenever the game state
/// should be updated, independently of GameCommands. Use the [GameRuntime::simulate()] function instead
/// of calling this directly in order to utilize automate change detection
pub trait SimRunner: Send + Sync {
    fn simulate(&mut self, world: &mut World);
}

/// A simple example game runner for a turn based game
pub struct TurnBasedGameRunner {
    pub turn_schedule: Schedule,
}

impl SimRunner for TurnBasedGameRunner {
    fn simulate(&mut self, world: &mut World) {
        self.turn_schedule.run(world);
    }
}

/// A simple example game runner for a real time based game
pub struct RealTimeGameRunner {
    pub ticks: usize,
    pub tick_schedule: Schedule,
}

impl SimRunner for RealTimeGameRunner {
    fn simulate(&mut self, world: &mut World) {
        self.ticks = self.ticks.saturating_add(1);
        self.tick_schedule.run(world);
    }
}
