//! Build the runtime scheme topology from a parsed `Module`.
//!
//! Each `SchemeDecl` becomes an `SchemeRuntime`. Each `FacetInstance` inside
//! the scheme becomes a `FacetRuntime` with its state fields initialized
//! from the facet declaration's init expressions, evaluated against an
//! empty environment (state inits are constants in v1).
//!
//! Emits `scheme_instantiated` + `facet_instantiated` events as the tree
//! is built. After this pass the runtime owns everything it needs — the
//! Module can be dropped or kept; the runtime no longer reads from it.

use std::collections::HashMap;

use urchin_parser::ast::{DispatchDecl, Handler, Module, FacetDecl};

use crate::env::FacetState;
use crate::events::{Event, EventSink};
use crate::interp::eval_init;

#[derive(Debug)]
pub struct Topology {
    pub schemes: Vec<SchemeRuntime>,
}

#[derive(Debug)]
pub struct SchemeRuntime {
    pub name: String,
    pub parent: Option<String>,
    pub io_spines: Vec<IoSpineBinding>,
    pub facets: Vec<FacetRuntime>,
    pub dispatch: Vec<DispatchDecl>,
}

#[derive(Debug)]
pub struct IoSpineBinding {
    /// Local spine name as declared in this scheme (e.g., "clock").
    pub name: String,
    /// Module path the spine points at (e.g., `["io", "sim", "clock"]`).
    pub io_path: Vec<String>,
}

#[derive(Debug)]
pub struct FacetRuntime {
    /// Facet-declaration name; also the facet-instance name in v1 (the
    /// grammar doesn't yet support multiple instances of the same facet
    /// in one scheme with disambiguating names).
    pub name: String,
    pub state: FacetState,
    pub handlers: Vec<Handler>,
    pub io_args: Vec<String>,
}

pub fn instantiate(module: &Module, sink: &mut dyn EventSink) -> Result<Topology, String> {
    let facet_index: HashMap<&str, &FacetDecl> = module
        .facets
        .iter()
        .map(|r| (r.name.as_str(), r))
        .collect();

    let mut schemes = Vec::with_capacity(module.schemes.len());
    for scheme in &module.schemes {
        sink.emit(Event::SchemeInstantiated {
            scheme: scheme.name.clone(),
            parent: scheme.parent.clone(),
        });

        let mut facets = Vec::with_capacity(scheme.facet_instances.len());
        for inst in &scheme.facet_instances {
            let facet_decl = facet_index
                .get(inst.name.as_str())
                .ok_or_else(|| format!("unknown facet `{}` in scheme `{}`", inst.name, scheme.name))?;

            let mut state = FacetState::new();
            for field in &facet_decl.state {
                let v = eval_init(&field.init, &state)?;
                state.set(&field.name, v);
            }

            let initial: Vec<_> = state
                .fields()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            sink.emit(Event::FacetInstantiated {
                scheme: scheme.name.clone(),
                instance: inst.name.clone(),
                state: initial,
            });

            facets.push(FacetRuntime {
                name: inst.name.clone(),
                state,
                handlers: facet_decl.handlers.clone(),
                io_args: inst.io_args.clone(),
            });
        }

        schemes.push(SchemeRuntime {
            name: scheme.name.clone(),
            parent: scheme.parent.clone(),
            io_spines: scheme
                .io_spines
                .iter()
                .map(|s| IoSpineBinding {
                    name: s.name.clone(),
                    io_path: s.io_path.clone(),
                })
                .collect(),
            facets,
            dispatch: scheme.dispatch.clone(),
        });
    }

    Ok(Topology { schemes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use urchin_parser::parse;

    use crate::events::VecSink;
    use crate::value::Value;

    #[test]
    fn instantiates_scheme_tree_from_agent_example() {
        let src = std::fs::read_to_string("../../examples/agent.urchin").expect("read example");
        let module = parse(&src).expect("parse");
        let mut sink = VecSink::default();
        let topo = instantiate(&module, &mut sink).expect("instantiate");

        // Two schemes: creativePersona (child) and rubberDuck (parent root).
        assert_eq!(topo.schemes.len(), 2);
        let names: Vec<&str> = topo.schemes.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"creativePersona"));
        assert!(names.contains(&"rubberDuck"));

        // creativePersona has 3 facet instances with initial state = 0.
        let cp = topo.schemes.iter().find(|a| a.name == "creativePersona").unwrap();
        assert_eq!(cp.facets.len(), 3);
        for facet in &cp.facets {
            // Each facet has exactly one state field initialized to Int(0).
            let fields: Vec<_> = facet.state.fields().collect();
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].1, &Value::Int(0));
        }
    }

    #[test]
    fn emits_scheme_and_facet_instantiated_events() {
        let src = "facet hunger { /// _state level = 0  /// _handlers on tick -> int { level } }
                   scheme mind { /// _io clock: io.sim.clock  /// _facets hunger(clock) }";
        let module = parse(src).expect("parse");
        let mut sink = VecSink::default();
        instantiate(&module, &mut sink).expect("instantiate");

        let scheme_events: Vec<_> = sink
            .events
            .iter()
            .filter(|e| matches!(e, Event::SchemeInstantiated { .. }))
            .collect();
        assert_eq!(scheme_events.len(), 1);

        let facet_events: Vec<_> = sink
            .events
            .iter()
            .filter(|e| matches!(e, Event::FacetInstantiated { .. }))
            .collect();
        assert_eq!(facet_events.len(), 1);

        if let Event::FacetInstantiated { scheme, instance, state } = facet_events[0] {
            assert_eq!(scheme, "mind");
            assert_eq!(instance, "hunger");
            assert_eq!(state.as_slice(), &[("level".to_string(), Value::Int(0))]);
        }
    }

    #[test]
    fn unknown_facet_in_scheme_is_an_error() {
        let src = "scheme mind { /// _io clock: io.sim.clock  /// _facets nope(clock) }";
        let module = parse(src).expect("parse");
        let mut sink = VecSink::default();
        let err = instantiate(&module, &mut sink).unwrap_err();
        assert!(err.contains("nope"), "got: {err}");
    }
}
