//! Urchin typechecker — dispatch coverage check.
//!
//! Per-scheme: when 2+ composed facets handle the same message type, the
//! scheme MUST declare an `on <spine>.<event> <mode>` dispatch
//! (parallel / async / sequence). The match-up between `spine.event`
//! and the message type is by name (event name == type name); good
//! enough until IO module signatures are formalized.
//!
//! The previous broadcast-completeness check was retired when the
//! `broadcast` verb was dropped from the language (REST-shaped
//! request/response model, no intra-scheme pub/sub).

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use urchin_parser::ast::{SchemeDecl, IoSpine, Module, FacetDecl};

/// A semantic check failure. Mirrors `urchin_parser::ParseError`'s shape
/// so the same ariadne pipeline can render both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckError {
    pub message: String,
    pub span: Range<usize>,
}

/// Run all checks on a module. Returns `Ok(())` if everything passes,
/// otherwise a list of `CheckError`s.
pub fn check(module: &Module) -> Result<(), Vec<CheckError>> {
    let facet_index: HashMap<&str, &FacetDecl> = module
        .facets
        .iter()
        .map(|r| (r.name.as_str(), r))
        .collect();

    let io_decl_names: HashSet<&str> = module
        .io_decls
        .iter()
        .map(|d| d.name.as_str())
        .collect();

    let mut errors = Vec::new();
    for scheme in &module.schemes {
        check_dispatch_coverage(scheme, &facet_index, &mut errors);
        check_io_spine_paths(scheme, &io_decl_names, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// When 2+ composed facets handle the same message type, the scheme must
/// declare an `on <spine>.<event> <mode>` dispatch — no implicit default.
/// The spine.event ↔ message-type match-up is by name (event name equals
/// type name); good enough until IO module signatures are formalized.
fn check_dispatch_coverage(
    scheme: &SchemeDecl,
    facet_index: &HashMap<&str, &FacetDecl>,
    errors: &mut Vec<CheckError>,
) {
    let composed: Vec<&FacetDecl> = scheme
        .facet_instances
        .iter()
        .filter_map(|inst| facet_index.get(inst.name.as_str()).copied())
        .collect();

    // Build map: message_type -> set of facet-instance names that handle it.
    let mut handlers_by_type: HashMap<Vec<String>, HashSet<String>> = HashMap::new();
    for r in &composed {
        for h in &r.handlers {
            handlers_by_type
                .entry(h.message_type.clone())
                .or_default()
                .insert(r.name.clone());
        }
    }

    // Set of message-type names the scheme's dispatch decls cover. Dispatch
    // events are `spine.event`; the `event` segment is the message type
    // name we match against.
    let dispatched_events: HashSet<&str> = scheme
        .dispatch
        .iter()
        .map(|d| d.event.event.as_str())
        .collect();

    for (msg_type, handler_facets) in &handlers_by_type {
        if handler_facets.len() < 2 {
            continue;
        }
        let leaf = msg_type.last().map(String::as_str).unwrap_or("");
        if !dispatched_events.contains(leaf) {
            let mut facets: Vec<&str> = handler_facets.iter().map(String::as_str).collect();
            facets.sort();
            errors.push(CheckError {
                message: format!(
                    "scheme `{}` composes {} facets that handle `{}` ({}); a dispatch declaration `on <spine>.{} <mode>` is required",
                    scheme.name,
                    handler_facets.len(),
                    msg_type.join("."),
                    facets.join(", "),
                    leaf,
                ),
                // Until SchemeDecl carries a span, point at file start. The
                // error message itself names the scheme and the missing
                // dispatch precisely.
                span: 0..0,
            });
        }
    }
}

/// Each `name: io.<path>` spine on an scheme must resolve to something
/// the language knows about. Two namespaces accept paths:
///
/// - `io.sim.*` — built-in simulation primitives, hardcoded in the runtime.
/// - `io.<name>` — must match an `io <name> { ... }` declaration in the
///   module.
///
/// Known wart: if a user declares `io sim { ... }` it'd collide with the
/// sim namespace. Not solved here; tracked for the IO grammar harden
/// pass.
fn check_io_spine_paths(
    scheme: &SchemeDecl,
    io_decl_names: &HashSet<&str>,
    errors: &mut Vec<CheckError>,
) {
    for spine in &scheme.io_spines {
        if !is_resolvable(spine, io_decl_names) {
            errors.push(CheckError {
                message: format!(
                    "scheme `{}` binds spine `{}: io.{}` to an unknown io path; declare `io {} {{ ... }}` or use a built-in `io.sim.*` primitive",
                    scheme.name,
                    spine.name,
                    spine.io_path[1..].join("."),
                    spine.io_path.get(1).cloned().unwrap_or_default(),
                ),
                // Until IoSpine carries a span, point at file start. The
                // error message names the scheme + spine + path precisely.
                span: 0..0,
            });
        }
    }
}

fn is_resolvable(spine: &IoSpine, io_decl_names: &HashSet<&str>) -> bool {
    // Path must start with the `io` namespace.
    if spine.io_path.first().map(String::as_str) != Some("io") {
        return false;
    }
    // `io.sim.*` — built-in primitive namespace.
    if spine.io_path.get(1).map(String::as_str) == Some("sim") {
        return true;
    }
    // `io.<name>` — must match a declared io decl. Multi-segment user
    // paths (`io.foo.bar`) aren't supported until module nesting lands.
    if spine.io_path.len() == 2 {
        if let Some(name) = spine.io_path.get(1) {
            return io_decl_names.contains(name.as_str());
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use urchin_parser::parse;

    #[test]
    fn empty_module_passes() {
        let m = parse("").expect("parse");
        check(&m).expect("nothing to check");
    }

    #[test]
    fn module_with_only_facets_passes() {
        let src = "facet hunger { /// _handlers on tick {} }";
        let m = parse(src).expect("parse");
        check(&m).expect("no schemes, no problem");
    }

    #[test]
    fn dispatch_passes_when_two_handlers_have_explicit_dispatch() {
        let src = "
            facet hunger { /// _handlers on tick {} }
            facet voice  { /// _handlers on tick {} }
            scheme mind {
              /// _io
              clock: io.sim.clock
              /// _facets
              hunger(clock)
              voice(clock)
              /// _dispatch_scripts
              on clock.tick sequence(hunger -> voice)
            }
        ";
        let m = parse(src).expect("parse");
        check(&m).expect("dispatch decl satisfies the rule");
    }

    #[test]
    fn dispatch_fails_when_two_handlers_lack_dispatch() {
        let src = "
            facet hunger { /// _handlers on tick {} }
            facet voice  { /// _handlers on tick {} }
            scheme mind {
              /// _io
              clock: io.sim.clock
              /// _facets
              hunger(clock)
              voice(clock)
            }
        ";
        let m = parse(src).expect("parse");
        let errs = check(&m).expect_err("should fail — no dispatch");
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("tick"));
        assert!(errs[0].message.contains("hunger"));
        assert!(errs[0].message.contains("voice"));
        assert!(errs[0].message.contains("dispatch"));
    }

    #[test]
    fn dispatch_passes_when_only_one_handler() {
        let src = "
            facet hunger { /// _handlers on tick {} }
            scheme mind {
              /// _io
              clock: io.sim.clock
              /// _facets
              hunger(clock)
            }
        ";
        let m = parse(src).expect("parse");
        check(&m).expect("one handler needs no dispatch");
    }

    #[test]
    fn dispatch_passes_with_three_handlers_and_sequence_chain() {
        let src = "
            facet a { /// _handlers on tick {} }
            facet b { /// _handlers on tick {} }
            facet c { /// _handlers on tick {} }
            scheme mind {
              /// _io
              clock: io.sim.clock
              /// _facets
              a(clock)
              b(clock)
              c(clock)
              /// _dispatch_scripts
              on clock.tick sequence(a -> b -> c)
            }
        ";
        let m = parse(src).expect("parse");
        check(&m).expect("three-stage sequence satisfies the rule");
    }

    #[test]
    fn dispatch_passes_with_parallel_mode() {
        let src = "
            facet a { /// _handlers on tick {} }
            facet b { /// _handlers on tick {} }
            scheme mind {
              /// _io
              clock: io.sim.clock
              /// _facets
              a(clock)
              b(clock)
              /// _dispatch_scripts
              on clock.tick parallel
            }
        ";
        let m = parse(src).expect("parse");
        check(&m).expect("parallel dispatch satisfies the rule");
    }

    #[test]
    fn spine_with_undeclared_io_path_errors() {
        // `io.frobnicator` doesn't match any io_decl in the module and
        // isn't a built-in sim primitive — should fail to check.
        let src = "
            scheme persona @ root {
              /// _io
              assistant: io.frobnicator
              /// _facets
            }
            scheme root {
              /// _io
              peers: io.sim.comms.peer
            }
        ";
        let m = parse(src).expect("parse");
        let errs = check(&m).expect_err("undeclared io path should fail");
        assert!(errs.iter().any(|e| e.message.contains("frobnicator")));
    }

    #[test]
    fn spine_with_user_declared_io_passes() {
        let src = "
            io llm {
              /// _requests
              ask(prompt: str) -> str
            }
            scheme persona @ root {
              /// _io
              assistant: io.llm
              /// _facets
            }
            scheme root {
              /// _io
              peers: io.sim.comms.peer
            }
        ";
        let m = parse(src).expect("parse");
        check(&m).expect("declared io spine should pass");
    }

    #[test]
    fn spine_with_sim_primitive_passes() {
        // `io.sim.clock` is a built-in primitive — no io_decl needed.
        let src = "
            scheme persona @ root {
              /// _io
              clock: io.sim.clock
              /// _facets
            }
            scheme root {
              /// _io
              peers: io.sim.comms.peer
            }
        ";
        let m = parse(src).expect("parse");
        check(&m).expect("sim primitive should pass");
    }

    #[test]
    fn dispatch_passes_with_async_mode() {
        let src = "
            facet a { /// _handlers on tick {} }
            facet b { /// _handlers on tick {} }
            scheme mind {
              /// _io
              clock: io.sim.clock
              /// _facets
              a(clock)
              b(clock)
              /// _dispatch_scripts
              on clock.tick async
            }
        ";
        let m = parse(src).expect("parse");
        check(&m).expect("async dispatch satisfies the rule");
    }
}
