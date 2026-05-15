//! Sim clock + dispatch driver.
//!
//! The sim clock is a hardcoded source that fires `clock.tick` events
//! for N iterations. For each tick:
//!   1. Emit a `tick` event with the tick number.
//!   2. For each actor whose io_spines bind a spine to `io.sim.clock`:
//!     - For each of that actor's dispatch decls where the event's
//!       spine name matches AND the event name is "tick":
//!       - Fan out per the dispatch mode (parallel only in v1) to every
//!         role-instance whose role declares a handler for "tick".
//!       - Each handler runs (state may mutate; events emit); the
//!         handler's return value becomes a `handler_return` event.
//!
//! "Parallel" in v1 is sequential-under-the-hood — we want determinism
//! over concurrency in the first slice. Async + sequence dispatch are
//! deferred until a use case demands them.

use urchin_parser::ast::DispatchMode;

use crate::events::{Event, EventSink};
use crate::instantiate::{ActorRuntime, Topology};
use crate::interp::run_handler;
use crate::value::Value;

/// Run a sim clock for `ticks` iterations against the topology. Drives
/// dispatch for each tick, mutating role state in place and emitting
/// events to `sink`.
pub fn run_sim(topology: &mut Topology, ticks: u64, sink: &mut dyn EventSink) -> Result<(), String> {
    for n in 0..ticks {
        sink.emit(Event::Tick { n });
        for actor in &mut topology.actors {
            dispatch_tick(actor, n, sink)?;
        }
    }
    Ok(())
}

fn dispatch_tick(actor: &mut ActorRuntime, _tick_n: u64, sink: &mut dyn EventSink) -> Result<(), String> {
    // Find spines bound to io.sim.clock — these are the channels through
    // which tick events arrive on this actor.
    let clock_spines: Vec<String> = actor
        .io_spines
        .iter()
        .filter(|s| s.io_path == ["io", "sim", "clock"])
        .map(|s| s.name.clone())
        .collect();
    if clock_spines.is_empty() {
        return Ok(()); // actor doesn't subscribe to sim clock
    }

    // Find dispatch decls matching `<clock_spine>.tick`.
    let dispatches: Vec<DispatchMode> = actor
        .dispatch
        .iter()
        .filter(|d| clock_spines.contains(&d.event.spine) && d.event.event == "tick")
        .map(|d| d.mode.clone())
        .collect();

    let actor_name = actor.name.clone();

    if dispatches.is_empty() {
        // No dispatch decl, but there might still be a single role-instance
        // that handles `tick`. Per the typechecker, single-handler case
        // doesn't require an explicit dispatch.
        run_handlers_for_tick(&actor_name, &mut actor.roles, /*restrict_to*/ None, sink)?;
        return Ok(());
    }

    for mode in dispatches {
        match mode {
            DispatchMode::Parallel | DispatchMode::Async => {
                // v1: both parallel and async run all matching handlers
                // sequentially. Async semantics (fire-and-forget) only
                // diverges from parallel once we have real concurrency.
                run_handlers_for_tick(&actor_name, &mut actor.roles, None, sink)?;
            }
            DispatchMode::Sequence(insts) => {
                // Restrict to the listed instances, in the listed order.
                run_handlers_for_tick(&actor_name, &mut actor.roles, Some(&insts), sink)?;
            }
        }
    }

    Ok(())
}

/// For each role-instance in the actor whose role has a handler for
/// message type "tick", run that handler. If `restrict_to` is `Some(list)`,
/// only run instances named in that list, in list order.
fn run_handlers_for_tick(
    actor_name: &str,
    roles: &mut [crate::instantiate::RoleRuntime],
    restrict_to: Option<&[String]>,
    sink: &mut dyn EventSink,
) -> Result<(), String> {
    let order: Vec<usize> = match restrict_to {
        Some(list) => list
            .iter()
            .filter_map(|name| roles.iter().position(|r| &r.name == name))
            .collect(),
        None => (0..roles.len()).collect(),
    };

    for idx in order {
        // Find a handler on this role for `on tick`.
        let role = &mut roles[idx];
        let instance_name = role.name.clone();
        let handler = role
            .handlers
            .iter()
            .find(|h| h.message_type == ["tick"])
            .cloned();
        let Some(handler) = handler else { continue };

        let value = run_handler(
            &handler,
            &mut role.state,
            None,
            actor_name,
            &instance_name,
            sink,
        )?;

        sink.emit(Event::HandlerReturn {
            actor: actor_name.to_string(),
            instance: instance_name,
            message: "tick".to_string(),
            value,
        });

        // Silence an "unused" warning when Value isn't otherwise referenced.
        let _ = &Value::Unit;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use urchin_parser::parse;

    use crate::events::VecSink;
    use crate::instantiate::instantiate;
    use crate::value::Value;

    fn topo(src: &str) -> Topology {
        let module = parse(src).expect("parse");
        let mut sink = VecSink::default();
        instantiate(&module, &mut sink).expect("instantiate")
    }

    #[test]
    fn one_tick_runs_every_handler_in_parallel_dispatch() {
        let src = std::fs::read_to_string("../../examples/agent.urchin").expect("read");
        let mut t = topo(&src);
        let mut sink = VecSink::default();
        run_sim(&mut t, 1, &mut sink).expect("sim runs");

        // After one tick, each of the three role-instances incremented its
        // sole state field from 0 to 1.
        let cp = t.actors.iter().find(|a| a.name == "creativePersona").unwrap();
        for r in &cp.roles {
            let f: Vec<_> = r.state.fields().collect();
            assert_eq!(f.len(), 1);
            assert_eq!(f[0].1, &Value::Int(1), "{} did not tick", r.name);
        }

        // Three handler_return events, all returning Int(1).
        let returns: Vec<_> = sink
            .events
            .iter()
            .filter_map(|e| match e {
                Event::HandlerReturn { instance, value, .. } => Some((instance.clone(), value.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(returns.len(), 3);
        assert!(returns.iter().all(|(_, v)| v == &Value::Int(1)));
    }

    #[test]
    fn five_ticks_increments_each_state_to_five() {
        let src = std::fs::read_to_string("../../examples/agent.urchin").expect("read");
        let mut t = topo(&src);
        let mut sink = VecSink::default();
        run_sim(&mut t, 5, &mut sink).expect("sim runs");

        let cp = t.actors.iter().find(|a| a.name == "creativePersona").unwrap();
        for r in &cp.roles {
            let f: Vec<_> = r.state.fields().collect();
            assert_eq!(f[0].1, &Value::Int(5), "{} expected 5", r.name);
        }

        // 5 tick events emitted.
        let ticks: Vec<_> = sink
            .events
            .iter()
            .filter(|e| matches!(e, Event::Tick { .. }))
            .collect();
        assert_eq!(ticks.len(), 5);
    }

    #[test]
    fn garden_arcade_example_runs_end_to_end() {
        // The GA-shaped seed corpus example. Three roles compose into a
        // feedUser; each has different increment rates (1, 2, 5); the
        // poster has a second state field (`isHot`) that flips via an
        // `if` guard once postsWritten passes the milestone. Five ticks
        // lands all three engagement counters at expected totals.
        let src = std::fs::read_to_string("../../examples/garden_arcade.urchin").expect("read");
        let mut t = topo(&src);
        let mut sink = VecSink::default();
        run_sim(&mut t, 5, &mut sink).expect("sim runs");

        let user = t.actors.iter().find(|a| a.name == "feedUser").expect("feedUser");

        let poster = user.roles.iter().find(|r| r.name == "poster").expect("poster");
        assert_eq!(poster.state.get("postsWritten"), Some(&Value::Int(5)));
        // milestone fires once postsWritten > 2 (i.e., starting from the 3rd tick)
        assert_eq!(poster.state.get("isHot"), Some(&Value::Int(1)));

        let reactor = user.roles.iter().find(|r| r.name == "reactor").expect("reactor");
        assert_eq!(reactor.state.get("reactionsGiven"), Some(&Value::Int(10)));

        let lurker = user.roles.iter().find(|r| r.name == "lurker").expect("lurker");
        assert_eq!(lurker.state.get("minutesScrolled"), Some(&Value::Int(25)));
    }

    #[test]
    fn root_actor_without_clock_spine_does_not_tick() {
        // rubberDuck has io.sim.comms.peer but no io.sim.clock — it
        // shouldn't receive tick events at all. (And it has no roles, so
        // there's nothing to run anyway, but the rule is: no clock spine,
        // no participation in sim clock dispatch.)
        let src = std::fs::read_to_string("../../examples/agent.urchin").expect("read");
        let mut t = topo(&src);
        let mut sink = VecSink::default();
        run_sim(&mut t, 3, &mut sink).expect("sim runs");

        let returns_for_rd: Vec<_> = sink
            .events
            .iter()
            .filter(|e| matches!(e, Event::HandlerReturn { actor, .. } if actor == "rubberDuck"))
            .collect();
        assert!(returns_for_rd.is_empty(), "rubberDuck has no clock spine");
    }
}
