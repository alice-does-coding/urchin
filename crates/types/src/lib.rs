//! Urchin typechecker — dispatch coverage check.
//!
//! Per-actor: when 2+ composed roles handle the same message type, the
//! actor MUST declare an `on <spine>.<event> <mode>` dispatch
//! (parallel / async / sequence). The match-up between `spine.event`
//! and the message type is by name (event name == type name); good
//! enough until IO module signatures are formalized.
//!
//! The previous broadcast-completeness check was retired when the
//! `broadcast` verb was dropped from the language (REST-shaped
//! request/response model, no intra-actor pub/sub).

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use urchin_parser::ast::{ActorDecl, Module, RoleDecl};

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
    let role_index: HashMap<&str, &RoleDecl> = module
        .roles
        .iter()
        .map(|r| (r.name.as_str(), r))
        .collect();

    let mut errors = Vec::new();
    for actor in &module.actors {
        check_dispatch_coverage(actor, &role_index, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// When 2+ composed roles handle the same message type, the actor must
/// declare an `on <spine>.<event> <mode>` dispatch — no implicit default.
/// The spine.event ↔ message-type match-up is by name (event name equals
/// type name); good enough until IO module signatures are formalized.
fn check_dispatch_coverage(
    actor: &ActorDecl,
    role_index: &HashMap<&str, &RoleDecl>,
    errors: &mut Vec<CheckError>,
) {
    let composed: Vec<&RoleDecl> = actor
        .role_instances
        .iter()
        .filter_map(|inst| role_index.get(inst.name.as_str()).copied())
        .collect();

    // Build map: message_type -> set of role-instance names that handle it.
    let mut handlers_by_type: HashMap<Vec<String>, HashSet<String>> = HashMap::new();
    for r in &composed {
        for h in &r.handlers {
            handlers_by_type
                .entry(h.message_type.clone())
                .or_default()
                .insert(r.name.clone());
        }
    }

    // Set of message-type names the actor's dispatch decls cover. Dispatch
    // events are `spine.event`; the `event` segment is the message type
    // name we match against.
    let dispatched_events: HashSet<&str> = actor
        .dispatch
        .iter()
        .map(|d| d.event.event.as_str())
        .collect();

    for (msg_type, handler_roles) in &handlers_by_type {
        if handler_roles.len() < 2 {
            continue;
        }
        let leaf = msg_type.last().map(String::as_str).unwrap_or("");
        if !dispatched_events.contains(leaf) {
            let mut roles: Vec<&str> = handler_roles.iter().map(String::as_str).collect();
            roles.sort();
            errors.push(CheckError {
                message: format!(
                    "actor `{}` composes {} roles that handle `{}` ({}); a dispatch declaration `on <spine>.{} <mode>` is required",
                    actor.name,
                    handler_roles.len(),
                    msg_type.join("."),
                    roles.join(", "),
                    leaf,
                ),
                // Until ActorDecl carries a span, point at file start. The
                // error message itself names the actor and the missing
                // dispatch precisely.
                span: 0..0,
            });
        }
    }
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
    fn module_with_only_roles_passes() {
        let src = "role hunger { /// _handlers on tick {} }";
        let m = parse(src).expect("parse");
        check(&m).expect("no actors, no problem");
    }

    #[test]
    fn dispatch_passes_when_two_handlers_have_explicit_dispatch() {
        let src = "
            role hunger { /// _handlers on tick {} }
            role voice  { /// _handlers on tick {} }
            actor mind {
              /// _io
              clock: io.sim.clock
              /// _roles
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
            role hunger { /// _handlers on tick {} }
            role voice  { /// _handlers on tick {} }
            actor mind {
              /// _io
              clock: io.sim.clock
              /// _roles
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
            role hunger { /// _handlers on tick {} }
            actor mind {
              /// _io
              clock: io.sim.clock
              /// _roles
              hunger(clock)
            }
        ";
        let m = parse(src).expect("parse");
        check(&m).expect("one handler needs no dispatch");
    }

    #[test]
    fn dispatch_passes_with_three_handlers_and_sequence_chain() {
        let src = "
            role a { /// _handlers on tick {} }
            role b { /// _handlers on tick {} }
            role c { /// _handlers on tick {} }
            actor mind {
              /// _io
              clock: io.sim.clock
              /// _roles
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
            role a { /// _handlers on tick {} }
            role b { /// _handlers on tick {} }
            actor mind {
              /// _io
              clock: io.sim.clock
              /// _roles
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
    fn dispatch_passes_with_async_mode() {
        let src = "
            role a { /// _handlers on tick {} }
            role b { /// _handlers on tick {} }
            actor mind {
              /// _io
              clock: io.sim.clock
              /// _roles
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
