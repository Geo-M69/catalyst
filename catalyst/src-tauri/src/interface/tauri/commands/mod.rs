pub mod auth;
pub(crate) mod blocking;
pub mod collections;
pub mod game_actions;
pub mod game_settings;
#[cfg(test)]
mod integration_tests;
pub mod library;
pub mod steam;
// Deprecated command helpers removed from source tree; keep module list
// minimal to avoid exposing unused code paths via the invoke handler.
