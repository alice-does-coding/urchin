//! Urchin runtime — tree-walking interpreter + sim scheduler.
//!
//! Milestone 1 surface:
//! - `Value` — the runtime value type (Int / Float / Str / List / Record / Unit)
//! - `interp` — evaluates `Stmt` and `Expr` against an `Env` + `FacetState`
//! - `instantiate` — builds the scheme tree from a parsed `Module`
//! - `schedule` — sim clock that fires `clock.tick` events and the dispatch
//!   driver that fans them out to facet-instances
//! - `events` — the structured JSON-Lines event log
//! - `run` — top-level driver; consumes a parsed `Module` and emits events
//!
//! Out of scope for milestone 1: algebraic effect handlers (sim primitives
//! are hardcoded), journal/replay, true concurrency, real IO, REPL,
//! async/sequence dispatch (parallel only).

pub mod value;
pub mod env;
pub mod interp;
pub mod events;
pub mod instantiate;
pub mod schedule;

use urchin_parser::ast::Module;

use crate::events::EventSink;

/// Top-level driver: instantiate a topology from `module`, run the sim
/// clock for `ticks` iterations, emit all events to `sink`.
pub fn run(module: &Module, ticks: u64, sink: &mut dyn EventSink) -> Result<(), String> {
    let mut topology = instantiate::instantiate(module, sink)?;
    schedule::run_sim(&mut topology, ticks, sink)
}
