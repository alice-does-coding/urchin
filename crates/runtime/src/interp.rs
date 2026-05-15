//! Tree-walking interpreter for facet handlers.
//!
//! Evaluates `Stmt` and `Expr` nodes against an `Env` (transient locals)
//! and a `FacetState` (persistent state fields). State mutation goes
//! through the state-field path — assigns where the LHS matches a known
//! state-field name emit a `state_assign` event; assigns to fresh names
//! become local bindings.
//!
//! The `~>` operator (`Expr::Binary(BinOp::StateShift, lhs, rhs)`) is
//! evaluated as "return the RHS." Both sides reference the OLD value of
//! the state field, since the swap happens at the enclosing `Assign`
//! statement level. The LHS is documentation; the RHS produces the new
//! value.
//!
//! Handler return is the trailing `ExprStmt`'s value — block-expression
//! semantics per slice 31. Other trailing statement kinds yield `Unit`.

use urchin_parser::ast::{BinOp, Expr, Handler, Stmt};

use crate::env::{Env, FacetState};
use crate::events::{Event, EventSink};
use crate::value::Value;

/// Run a handler. Returns the handler's value (block-expression style)
/// or `Unit` if the body has no trailing expression.
pub fn run_handler(
    handler: &Handler,
    state: &mut FacetState,
    msg_binding: Option<Value>,
    scheme: &str,
    instance: &str,
    sink: &mut dyn EventSink,
) -> Result<Value, String> {
    let mut env = Env::new();
    if let (Some(name), Some(val)) = (&handler.binding, msg_binding) {
        env.set(name, val);
    }

    let n = handler.body.len();
    let mut tail_value = Value::Unit;
    for (i, stmt) in handler.body.iter().enumerate() {
        let v = run_stmt(stmt, &mut env, state, scheme, instance, sink)?;
        if i == n - 1 {
            if let Stmt::ExprStmt(_) = stmt {
                tail_value = v;
            }
        }
    }
    Ok(tail_value)
}

fn run_stmt(
    stmt: &Stmt,
    env: &mut Env,
    state: &mut FacetState,
    scheme: &str,
    instance: &str,
    sink: &mut dyn EventSink,
) -> Result<Value, String> {
    match stmt {
        Stmt::Assign { name, value } => {
            let new = eval_expr(value, env, state)?;
            if state.contains(name) {
                let old = state.get(name).cloned().unwrap_or(Value::Unit);
                state.set(name, new.clone());
                sink.emit(Event::StateAssign {
                    scheme: scheme.to_string(),
                    instance: instance.to_string(),
                    field: name.clone(),
                    old,
                    new,
                });
            } else {
                env.set(name, new);
            }
            Ok(Value::Unit)
        }
        Stmt::If { cond, then_body, else_body } => {
            let c = eval_expr(cond, env, state)?;
            let branch = if is_truthy(&c) { Some(then_body) } else { else_body.as_ref() };
            if let Some(body) = branch {
                for s in body {
                    run_stmt(s, env, state, scheme, instance, sink)?;
                }
            }
            Ok(Value::Unit)
        }
        Stmt::ExprStmt(e) => eval_expr(e, env, state),
    }
}

fn eval_expr(e: &Expr, env: &Env, state: &FacetState) -> Result<Value, String> {
    match e {
        Expr::Int(n) => Ok(Value::Int(*n)),
        Expr::Float(n) => Ok(Value::Float(*n)),
        Expr::Str(s) => Ok(Value::Str(s.clone())),
        Expr::Ident(n) => env
            .get(n)
            .or_else(|| state.get(n))
            .cloned()
            .ok_or_else(|| format!("unbound name: {n}")),
        Expr::Binary(op, l, r) => {
            // `~>` is syntactic flavor at the expression level — return the RHS.
            // (The state-swap semantics live at the enclosing Assign statement.)
            if matches!(op, BinOp::StateShift) {
                return eval_expr(r, env, state);
            }
            let lv = eval_expr(l, env, state)?;
            let rv = eval_expr(r, env, state)?;
            apply_binop(*op, lv, rv)
        }
        Expr::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                out.push(eval_expr(it, env, state)?);
            }
            Ok(Value::List(out))
        }
        Expr::FieldAccess { object, field } => {
            let obj = eval_expr(object, env, state)?;
            match obj {
                Value::Record(fields) => fields
                    .into_iter()
                    .find(|(n, _)| n == field)
                    .map(|(_, v)| v)
                    .ok_or_else(|| format!("no field `{field}` on record")),
                other => Err(format!("field access on non-record: {other:?}")),
            }
        }
        Expr::Call { callee, args: _ } => {
            // Milestone 1 has no built-in callables and no facet-instance call
            // semantics. Any call in a handler body is currently an error;
            // the example we're targeting doesn't use any.
            Err(format!("call to `{callee}` not supported in milestone-1 runtime"))
        }
        Expr::Match { .. } => Err("match expressions not supported in milestone-1 runtime".into()),
    }
}

fn apply_binop(op: BinOp, l: Value, r: Value) -> Result<Value, String> {
    use Value::{Float, Int};
    match (op, l, r) {
        (BinOp::Add, Int(a), Int(b)) => Ok(Int(a + b)),
        (BinOp::Sub, Int(a), Int(b)) => Ok(Int(a - b)),
        (BinOp::Mul, Int(a), Int(b)) => Ok(Int(a * b)),
        (BinOp::Div, Int(a), Int(b)) => Ok(Int(a / b)),
        (BinOp::Add, Float(a), Float(b)) => Ok(Float(a + b)),
        (BinOp::Sub, Float(a), Float(b)) => Ok(Float(a - b)),
        (BinOp::Mul, Float(a), Float(b)) => Ok(Float(a * b)),
        (BinOp::Div, Float(a), Float(b)) => Ok(Float(a / b)),
        (BinOp::Lt, Int(a), Int(b)) => Ok(Int((a < b) as i64)),
        (BinOp::Gt, Int(a), Int(b)) => Ok(Int((a > b) as i64)),
        (BinOp::Eq, Int(a), Int(b)) => Ok(Int((a == b) as i64)),
        (BinOp::Lt, Float(a), Float(b)) => Ok(Int((a < b) as i64)),
        (BinOp::Gt, Float(a), Float(b)) => Ok(Int((a > b) as i64)),
        (op, l, r) => Err(format!("unsupported op {op:?} on {l:?} / {r:?}")),
    }
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Int(0) => false,
        Value::Unit => false,
        _ => true,
    }
}

// --- Convenience for callers and tests --------------------------------

/// Set a state field, used by the instantiator + tests when bootstrapping
/// a facet's initial state.
pub fn eval_init(init: &Expr, state: &FacetState) -> Result<Value, String> {
    let env = Env::new();
    eval_expr(init, &env, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use urchin_parser::parse;

    use crate::events::VecSink;

    fn first_handler(src: &str) -> Handler {
        let module = parse(src).expect("parse");
        let facet = module.facets.into_iter().next().expect("facet");
        facet.handlers.into_iter().next().expect("handler")
    }

    fn init_state(src: &str) -> FacetState {
        let module = parse(src).expect("parse");
        let facet = &module.facets[0];
        let mut state = FacetState::new();
        for f in &facet.state {
            let v = eval_init(&f.init, &state).expect("init evaluates");
            state.set(&f.name, v);
        }
        state
    }

    #[test]
    fn evaluates_int_literal_handler() {
        let h = first_handler("facet X { /// _handlers on Tick -> int { 7 } }");
        let mut state = FacetState::new();
        let mut sink = VecSink::default();
        let v = run_handler(&h, &mut state, None, "a", "i", &mut sink).unwrap();
        assert_eq!(v, Value::Int(7));
        assert!(sink.events.is_empty());
    }

    #[test]
    fn arithmetic_returns_correct_value() {
        let h = first_handler("facet X { /// _handlers on Tick -> int { 1 + 2 * 3 } }");
        let mut state = FacetState::new();
        let mut sink = VecSink::default();
        let v = run_handler(&h, &mut state, None, "a", "i", &mut sink).unwrap();
        assert_eq!(v, Value::Int(7));
    }

    #[test]
    fn state_shift_increments_and_returns() {
        // The canonical agent.urchin facet shape.
        let src = "facet X {
                     /// _state
                     shotsTaken = 0

                     /// _handlers
                     on tick -> int {
                       shotsTaken = shotsTaken ~> shotsTaken + 1
                       shotsTaken
                     }
                   }";
        let h = first_handler(src);
        let mut state = init_state(src);
        let mut sink = VecSink::default();

        let v = run_handler(&h, &mut state, None, "a", "i", &mut sink).unwrap();
        assert_eq!(v, Value::Int(1));
        assert_eq!(state.get("shotsTaken"), Some(&Value::Int(1)));

        // Re-run: state persists across invocations.
        let v2 = run_handler(&h, &mut state, None, "a", "i", &mut sink).unwrap();
        assert_eq!(v2, Value::Int(2));
        assert_eq!(state.get("shotsTaken"), Some(&Value::Int(2)));
    }

    #[test]
    fn state_shift_emits_state_assign_event() {
        let src = "facet X {
                     /// _state shotsTaken = 0
                     /// _handlers on tick -> int { shotsTaken = shotsTaken ~> shotsTaken + 1  shotsTaken }
                   }";
        let h = first_handler(src);
        let mut state = init_state(src);
        let mut sink = VecSink::default();
        run_handler(&h, &mut state, None, "creativePersona", "photographer", &mut sink).unwrap();

        let assigns: Vec<_> = sink
            .events
            .iter()
            .filter(|e| matches!(e, Event::StateAssign { .. }))
            .collect();
        assert_eq!(assigns.len(), 1);
        if let Event::StateAssign { scheme, instance, field, old, new } = assigns[0] {
            assert_eq!(scheme, "creativePersona");
            assert_eq!(instance, "photographer");
            assert_eq!(field, "shotsTaken");
            assert_eq!(*old, Value::Int(0));
            assert_eq!(*new, Value::Int(1));
        }
    }

    #[test]
    fn local_binding_does_not_emit_state_assign() {
        // `tmp = ...` for a name that ISN'T a state field is a local binding;
        // no state_assign event.
        let src = "facet X {
                     /// _state shotsTaken = 0
                     /// _handlers on tick -> int { tmp = shotsTaken + 1  tmp }
                   }";
        let h = first_handler(src);
        let mut state = init_state(src);
        let mut sink = VecSink::default();
        let v = run_handler(&h, &mut state, None, "a", "i", &mut sink).unwrap();
        assert_eq!(v, Value::Int(1));
        assert!(
            !sink.events.iter().any(|e| matches!(e, Event::StateAssign { .. })),
            "local binding should not emit state_assign"
        );
        // State field unchanged.
        assert_eq!(state.get("shotsTaken"), Some(&Value::Int(0)));
    }

    #[test]
    fn if_branch_runs_when_truthy() {
        let src = "facet X {
                     /// _state level = 0
                     /// _handlers on tick -> int {
                       if 1 > 0 { level = level ~> 42 }
                       level
                     }
                   }";
        let h = first_handler(src);
        let mut state = init_state(src);
        let mut sink = VecSink::default();
        let v = run_handler(&h, &mut state, None, "a", "i", &mut sink).unwrap();
        assert_eq!(v, Value::Int(42));
    }

    #[test]
    fn if_branch_skipped_when_falsy() {
        let src = "facet X {
                     /// _state level = 0
                     /// _handlers on tick -> int {
                       if 0 > 1 { level = level ~> 42 }
                       level
                     }
                   }";
        let h = first_handler(src);
        let mut state = init_state(src);
        let mut sink = VecSink::default();
        let v = run_handler(&h, &mut state, None, "a", "i", &mut sink).unwrap();
        assert_eq!(v, Value::Int(0));
    }

    #[test]
    fn handler_without_trailing_expr_returns_unit() {
        let src = "facet X {
                     /// _state x = 0
                     /// _handlers on tick { x = x ~> 1 }
                   }";
        let h = first_handler(src);
        let mut state = init_state(src);
        let mut sink = VecSink::default();
        let v = run_handler(&h, &mut state, None, "a", "i", &mut sink).unwrap();
        assert_eq!(v, Value::Unit);
        assert_eq!(state.get("x"), Some(&Value::Int(1)));
    }
}
