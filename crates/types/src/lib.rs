//! Urchin typechecker — first slice: composition completeness.
//!
//! For each actor, find every message type that can flow through it
//! (broadcasts emitted by composed roles + handlers declared by composed
//! roles), and require that every emitted broadcast has at least one
//! handler in some composed role of the same actor. If not, error.
//!
//! IO-spine events aren't yet checked because the parser doesn't carry
//! a model of which events each `io.<kind>.*` module produces. That
//! lands when the IO module signatures are formalized (see SPEC §6).

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use urchin_parser::ast::{ActorDecl, Module, RoleDecl, Stmt};

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
        check_composition_completeness(actor, &role_index, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// For every broadcast emitted by any composed role in `actor`, require
/// that the same `actor` composes at least one role whose handlers
/// declare the broadcast's message type.
///
/// This catches the silent-event-loss bug — broadcasting onto an empty
/// bus is almost always a mistake.
fn check_composition_completeness(
    actor: &ActorDecl,
    role_index: &HashMap<&str, &RoleDecl>,
    errors: &mut Vec<CheckError>,
) {
    // Resolve each composed instance to its role declaration. An instance
    // name is the case-shifted role name; under the all-camelCase convention
    // they're literally identical.
    let composed: Vec<&RoleDecl> = actor
        .role_instances
        .iter()
        .filter_map(|inst| role_index.get(inst.name.as_str()).copied())
        .collect();

    // Set of message types this actor's composed roles can handle.
    let mut handled: HashSet<Vec<String>> = HashSet::new();
    for r in &composed {
        for h in &r.handlers {
            handled.insert(h.message_type.clone());
        }
    }

    // Walk every broadcast in every handler body of every composed role.
    // Any unhandled broadcast is an error pointing at the broadcast itself.
    for r in &composed {
        for h in &r.handlers {
            for stmt in &h.body {
                walk_broadcasts(stmt, &mut |msg_type, span| {
                    if !handled.contains(msg_type) {
                        errors.push(CheckError {
                            message: format!(
                                "actor `{}` broadcasts `{}` from role `{}` but no composed role handles it",
                                actor.name,
                                msg_type.join("."),
                                r.name,
                            ),
                            span: span.clone(),
                        });
                    }
                });
            }
        }
    }
}

/// Walk a statement tree and call `f` for every broadcast — passing both
/// the message type and the broadcast statement's source span.
fn walk_broadcasts<F: FnMut(&Vec<String>, &Range<usize>)>(stmt: &Stmt, f: &mut F) {
    match stmt {
        Stmt::Broadcast { message_type, span, .. } => f(message_type, span),
        Stmt::If {
            then_body,
            else_body,
            ..
        } => {
            for s in then_body {
                walk_broadcasts(s, f);
            }
            if let Some(eb) = else_body {
                for s in eb {
                    walk_broadcasts(s, f);
                }
            }
        }
        // Match arms can also contain broadcasts via Stmt::ExprStmt(Match{...});
        // walking through expression-level statement bodies lands when match
        // arm-body recursion is added in a follow-up.
        Stmt::Assign { .. } | Stmt::Reply(_) | Stmt::ExprStmt(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use urchin_parser::parse;

    #[test]
    fn passes_when_broadcast_is_handled_in_same_actor() {
        let src = "
            role hunger {
              on tick {
                broadcast wants
              }
            }
            role voice {
              on wants {}
            }
            actor mind {
              clock: io.sim.clock
              hunger(clock)
              voice(clock)
            }
        ";
        let m = parse(src).expect("parse");
        check(&m).expect("should pass");
    }

    #[test]
    fn fails_when_broadcast_has_no_handler() {
        let src = "
            role hunger {
              on tick {
                broadcast wants
              }
            }
            actor mind {
              clock: io.sim.clock
              hunger(clock)
            }
        ";
        let m = parse(src).expect("parse");
        let errs = check(&m).expect_err("should fail");
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("wants"));
        assert!(errs[0].message.contains("hunger"));
        assert!(errs[0].message.contains("mind"));
    }

    #[test]
    fn fails_for_broadcast_inside_if() {
        let src = "
            role hunger {
              ~ level: int
              on tick {
                if level > 0 { broadcast wants }
              }
            }
            actor mind {
              clock: io.sim.clock
              hunger(clock)
            }
        ";
        let m = parse(src).expect("parse");
        let errs = check(&m).expect_err("should fail");
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn handler_in_a_role_not_composed_does_not_count() {
        // An unrelated role declares a handler for `wants` but isn't composed
        // into the actor — should NOT satisfy the requirement.
        let src = "
            role hunger {
              on tick { broadcast wants }
            }
            role unrelated {
              on wants {}
            }
            actor mind {
              clock: io.sim.clock
              hunger(clock)
            }
        ";
        let m = parse(src).expect("parse");
        check(&m).expect_err("should fail — unrelated isn't composed");
    }

    #[test]
    fn empty_module_passes() {
        let m = parse("").expect("parse");
        check(&m).expect("nothing to check");
    }

    #[test]
    fn module_with_only_roles_passes() {
        let src = "role hunger { on tick { broadcast wants } }";
        let m = parse(src).expect("parse");
        // No actor → no composition to check.
        check(&m).expect("no actors, no problem");
    }

    #[test]
    fn multiple_unhandled_broadcasts_each_error() {
        let src = "
            role chatter {
              on tick {
                broadcast hum
                broadcast whisper
                broadcast shout
              }
            }
            actor mind {
              clock: io.sim.clock
              chatter(clock)
            }
        ";
        let m = parse(src).expect("parse");
        let errs = check(&m).expect_err("should fail");
        assert_eq!(errs.len(), 3);
    }
}
