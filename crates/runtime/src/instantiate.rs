//! Build the runtime actor topology from a parsed `Module`.
//!
//! Each `ActorDecl` becomes an `ActorRuntime`. Each `RoleInstance` inside
//! the actor becomes a `RoleRuntime` with its state fields initialized
//! from the role declaration's init expressions, evaluated against an
//! empty environment (state inits are constants in v1).
//!
//! Emits `actor_instantiated` + `role_instantiated` events as the tree
//! is built. After this pass the runtime owns everything it needs — the
//! Module can be dropped or kept; the runtime no longer reads from it.

use std::collections::HashMap;

use urchin_parser::ast::{DispatchDecl, Handler, Module, RoleDecl};

use crate::env::RoleState;
use crate::events::{Event, EventSink};
use crate::interp::eval_init;

#[derive(Debug)]
pub struct Topology {
    pub actors: Vec<ActorRuntime>,
}

#[derive(Debug)]
pub struct ActorRuntime {
    pub name: String,
    pub parent: Option<String>,
    pub io_spines: Vec<IoSpineBinding>,
    pub roles: Vec<RoleRuntime>,
    pub dispatch: Vec<DispatchDecl>,
}

#[derive(Debug)]
pub struct IoSpineBinding {
    /// Local spine name as declared in this actor (e.g., "clock").
    pub name: String,
    /// Module path the spine points at (e.g., `["io", "sim", "clock"]`).
    pub io_path: Vec<String>,
}

#[derive(Debug)]
pub struct RoleRuntime {
    /// Role-declaration name; also the role-instance name in v1 (the
    /// grammar doesn't yet support multiple instances of the same role
    /// in one actor with disambiguating names).
    pub name: String,
    pub state: RoleState,
    pub handlers: Vec<Handler>,
    pub io_args: Vec<String>,
}

pub fn instantiate(module: &Module, sink: &mut dyn EventSink) -> Result<Topology, String> {
    let role_index: HashMap<&str, &RoleDecl> = module
        .roles
        .iter()
        .map(|r| (r.name.as_str(), r))
        .collect();

    let mut actors = Vec::with_capacity(module.actors.len());
    for actor in &module.actors {
        sink.emit(Event::ActorInstantiated {
            actor: actor.name.clone(),
            parent: actor.parent.clone(),
        });

        let mut roles = Vec::with_capacity(actor.role_instances.len());
        for inst in &actor.role_instances {
            let role_decl = role_index
                .get(inst.name.as_str())
                .ok_or_else(|| format!("unknown role `{}` in actor `{}`", inst.name, actor.name))?;

            let mut state = RoleState::new();
            for field in &role_decl.state {
                let v = eval_init(&field.init, &state)?;
                state.set(&field.name, v);
            }

            let initial: Vec<_> = state
                .fields()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            sink.emit(Event::RoleInstantiated {
                actor: actor.name.clone(),
                instance: inst.name.clone(),
                state: initial,
            });

            roles.push(RoleRuntime {
                name: inst.name.clone(),
                state,
                handlers: role_decl.handlers.clone(),
                io_args: inst.io_args.clone(),
            });
        }

        actors.push(ActorRuntime {
            name: actor.name.clone(),
            parent: actor.parent.clone(),
            io_spines: actor
                .io_spines
                .iter()
                .map(|s| IoSpineBinding {
                    name: s.name.clone(),
                    io_path: s.io_path.clone(),
                })
                .collect(),
            roles,
            dispatch: actor.dispatch.clone(),
        });
    }

    Ok(Topology { actors })
}

#[cfg(test)]
mod tests {
    use super::*;
    use urchin_parser::parse;

    use crate::events::VecSink;
    use crate::value::Value;

    #[test]
    fn instantiates_actor_tree_from_agent_example() {
        let src = std::fs::read_to_string("../../examples/agent.urchin").expect("read example");
        let module = parse(&src).expect("parse");
        let mut sink = VecSink::default();
        let topo = instantiate(&module, &mut sink).expect("instantiate");

        // Two actors: creativePersona (child) and rubberDuck (parent root).
        assert_eq!(topo.actors.len(), 2);
        let names: Vec<&str> = topo.actors.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"creativePersona"));
        assert!(names.contains(&"rubberDuck"));

        // creativePersona has 3 role instances with initial state = 0.
        let cp = topo.actors.iter().find(|a| a.name == "creativePersona").unwrap();
        assert_eq!(cp.roles.len(), 3);
        for role in &cp.roles {
            // Each role has exactly one state field initialized to Int(0).
            let fields: Vec<_> = role.state.fields().collect();
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].1, &Value::Int(0));
        }
    }

    #[test]
    fn emits_actor_and_role_instantiated_events() {
        let src = "role hunger { /// _state level = 0  /// _handlers on tick -> int { level } }
                   actor mind { /// _io clock: io.sim.clock  /// _roles hunger(clock) }";
        let module = parse(src).expect("parse");
        let mut sink = VecSink::default();
        instantiate(&module, &mut sink).expect("instantiate");

        let actor_events: Vec<_> = sink
            .events
            .iter()
            .filter(|e| matches!(e, Event::ActorInstantiated { .. }))
            .collect();
        assert_eq!(actor_events.len(), 1);

        let role_events: Vec<_> = sink
            .events
            .iter()
            .filter(|e| matches!(e, Event::RoleInstantiated { .. }))
            .collect();
        assert_eq!(role_events.len(), 1);

        if let Event::RoleInstantiated { actor, instance, state } = role_events[0] {
            assert_eq!(actor, "mind");
            assert_eq!(instance, "hunger");
            assert_eq!(state.as_slice(), &[("level".to_string(), Value::Int(0))]);
        }
    }

    #[test]
    fn unknown_role_in_actor_is_an_error() {
        let src = "actor mind { /// _io clock: io.sim.clock  /// _roles nope(clock) }";
        let module = parse(src).expect("parse");
        let mut sink = VecSink::default();
        let err = instantiate(&module, &mut sink).unwrap_err();
        assert!(err.contains("nope"), "got: {err}");
    }
}
