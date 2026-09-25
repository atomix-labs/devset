//! End to end: the `devset` binary against real directories and git repositories.
//!
//! A file per area. Each test runs in its own [`Sandbox`](sandbox::Sandbox) and checks its
//! log against a snapshot in `snapshots/`.
#![cfg(test)]

mod apply;
mod conflicts;
mod errors;
mod layers;
mod output;
mod parts;
mod sandbox;
mod settings;
mod target;
mod templates;
mod update;
