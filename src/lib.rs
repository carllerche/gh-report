pub mod cli;
pub mod config;
pub mod state;
pub mod github;
pub mod report;
pub mod claude;
pub mod intelligence;

// Modules to be implemented in future milestones
// pub mod cache;   // Milestone 7

pub use config::Config;
pub use state::State;