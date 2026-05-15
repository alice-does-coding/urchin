//! Pretty printer — `Module` → formatted Urchin source.
//!
//! Output is canonical (deterministic; idempotent under re-formatting) and
//! round-trip safe (re-parsing produces an equal AST). Two-space indent,
//! brace-on-same-line, sections inside facets and schemes separated by a
//! blank line.
//!
//! **Lossy w.r.t. comments.** `///` comments are stripped by the lexer and
//! aren't carried in the AST, so the formatter doesn't preserve them.
//! Carrying comments through the AST is a follow-up.

use std::fmt::Write;

use crate::ast::{
    SchemeDecl, BinOp, CallArg, DispatchMode, Expr, Handler, IoDecl, Module,
    Pattern, FacetDecl, FacetInstance, Stmt, TypeExpr,
};

const INDENT: &str = "  ";

/// Format a whole module as Urchin source.
///
/// Output order follows the file-flow convention (small-to-large): facets
/// first (atomic primitives), then io decls (library-shaped contracts),
/// then schemes (orchestration). Within each group, declarations keep their
/// source order.
pub fn format(module: &Module) -> String {
    let mut out = String::new();
    let mut first = true;

    for facet in &module.facets {
        if !first {
            out.push_str("\n\n");
        }
        first = false;
        write_facet(&mut out, facet);
    }
    for io in &module.io_decls {
        if !first {
            out.push_str("\n\n");
        }
        first = false;
        write_io_decl(&mut out, io);
    }
    for scheme in &module.schemes {
        if !first {
            out.push_str("\n\n");
        }
        first = false;
        write_scheme(&mut out, scheme);
    }

    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn write_indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str(INDENT);
    }
}

// --- Facet ---------------------------------------------------------------

fn write_facet(out: &mut String, r: &FacetDecl) {
    write!(out, "facet {} {{", r.name).unwrap();

    let has_state = !r.state.is_empty();
    let has_handlers = !r.handlers.is_empty();
    let empty = !has_state && !has_handlers;

    if empty {
        out.push('}');
        return;
    }

    out.push('\n');

    if has_state {
        write_indent(out, 1);
        out.push_str("/// _state\n");
        for f in &r.state {
            write_indent(out, 1);
            write!(out, "{}", f.name).unwrap();
            if let Some(ty) = &f.ty {
                out.push_str(": ");
                write_type(out, ty);
            }
            out.push_str(" = ");
            write_expr(out, &f.init, 1);
            out.push('\n');
        }
    }

    if has_handlers {
        if has_state {
            out.push('\n');
        }
        write_indent(out, 1);
        out.push_str("/// _handlers\n");
        let mut first = true;
        for h in &r.handlers {
            if !first {
                out.push('\n');
            }
            first = false;
            write_handler(out, h, 1);
        }
    }

    out.push('}');
}

fn write_handler(out: &mut String, h: &Handler, depth: usize) {
    write_indent(out, depth);
    out.push_str("on ");
    write_dotted(out, &h.message_type);
    if let Some(b) = &h.binding {
        write!(out, " {}", b).unwrap();
    }
    if let Some(ret) = &h.return_ty {
        out.push_str(" -> ");
        write_type(out, ret);
    }
    write_block(out, &h.body, depth);
    out.push('\n');
}

// --- Scheme --------------------------------------------------------------

fn write_scheme(out: &mut String, a: &SchemeDecl) {
    write!(out, "scheme {}", a.name).unwrap();
    if let Some(parent) = &a.parent {
        write!(out, " @ {parent}").unwrap();
    }
    out.push_str(" {");

    let has_spines = !a.io_spines.is_empty();
    let has_instances = !a.facet_instances.is_empty();
    let has_dispatch = !a.dispatch.is_empty();
    let empty = !has_spines && !has_instances && !has_dispatch;

    if empty {
        out.push('}');
        return;
    }

    out.push('\n');

    if has_spines {
        write_indent(out, 1);
        out.push_str("/// _io\n");
        for s in &a.io_spines {
            write_indent(out, 1);
            write!(out, "{}: ", s.name).unwrap();
            write_dotted(out, &s.io_path);
            out.push('\n');
        }
    }

    if has_instances {
        if has_spines {
            out.push('\n');
        }
        write_indent(out, 1);
        out.push_str("/// _facets\n");
        for inst in &a.facet_instances {
            write_facet_instance(out, inst, 1);
            out.push('\n');
        }
    }

    if has_dispatch {
        if has_spines || has_instances {
            out.push('\n');
        }
        write_indent(out, 1);
        out.push_str("/// _dispatch_scripts\n");
        for d in &a.dispatch {
            write_indent(out, 1);
            write!(out, "on {}.{} ", d.event.spine, d.event.event).unwrap();
            write_dispatch_mode(out, &d.mode);
            out.push('\n');
        }
    }

    out.push('}');
}

fn write_facet_instance(out: &mut String, inst: &FacetInstance, depth: usize) {
    write_indent(out, depth);
    out.push_str(&inst.name);
    out.push('(');
    for (i, arg) in inst.io_args.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(arg);
    }
    out.push(')');
}

fn write_dispatch_mode(out: &mut String, mode: &DispatchMode) {
    match mode {
        DispatchMode::Parallel => out.push_str("parallel"),
        DispatchMode::Async => out.push_str("async"),
        DispatchMode::Sequence(insts) => {
            out.push_str("sequence(");
            for (i, name) in insts.iter().enumerate() {
                if i > 0 {
                    out.push_str(" -> ");
                }
                out.push_str(name);
            }
            out.push(')');
        }
    }
}

// --- IO declarations ----------------------------------------------------

fn write_io_decl(out: &mut String, io: &IoDecl) {
    write!(out, "io {} {{", io.name).unwrap();

    let has_requests = !io.requests.is_empty();
    let has_events = !io.events.is_empty();
    let has_contracts = !io.api_contracts.is_empty();
    let has_config = !io.config.is_empty();
    let empty = !has_requests && !has_events && !has_contracts && !has_config;

    if empty {
        out.push('}');
        return;
    }

    out.push('\n');
    let mut prior_section = false;

    if has_requests {
        write_indent(out, 1);
        out.push_str("/// _requests\n");
        for op in &io.requests {
            write_indent(out, 1);
            write_io_operation(out, op);
            out.push('\n');
        }
        prior_section = true;
    }

    if has_events {
        if prior_section { out.push('\n'); }
        write_indent(out, 1);
        out.push_str("/// _events\n");
        for op in &io.events {
            write_indent(out, 1);
            write_io_operation(out, op);
            out.push('\n');
        }
        prior_section = true;
    }

    if has_contracts {
        if prior_section { out.push('\n'); }
        write_indent(out, 1);
        out.push_str("/// _api_contracts\n");
        for alias in &io.api_contracts {
            write_indent(out, 1);
            write!(out, "{}: ", alias.name).unwrap();
            write_type(out, &alias.ty);
            out.push('\n');
        }
        prior_section = true;
    }

    if has_config {
        if prior_section { out.push('\n'); }
        write_indent(out, 1);
        out.push_str("/// _config\n");
        for c in &io.config {
            write_indent(out, 1);
            write!(out, "{} = ", c.name).unwrap();
            write_expr(out, &c.init, 1);
            out.push('\n');
        }
    }

    out.push('}');
}

fn write_io_operation(out: &mut String, op: &crate::ast::IoOperation) {
    write!(out, "{}(", op.name).unwrap();
    for (i, arg) in op.args.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        write!(out, "{}: ", arg.name).unwrap();
        write_type(out, &arg.ty);
    }
    out.push(')');
    if let Some(ret) = &op.return_type {
        out.push_str(" -> ");
        write_type(out, ret);
    }
}

// --- Statements & expressions ------------------------------------------

fn write_block(out: &mut String, stmts: &[Stmt], depth: usize) {
    if stmts.is_empty() {
        out.push_str(" {}");
        return;
    }
    out.push_str(" {\n");
    for s in stmts {
        write_stmt(out, s, depth + 1);
        out.push('\n');
    }
    write_indent(out, depth);
    out.push('}');
}

fn write_stmt(out: &mut String, s: &Stmt, depth: usize) {
    write_indent(out, depth);
    match s {
        Stmt::Assign { name, value } => {
            write!(out, "{} = ", name).unwrap();
            write_expr(out, value, depth);
        }
        Stmt::If { cond, then_body, else_body } => {
            out.push_str("if ");
            write_expr(out, cond, depth);
            write_block(out, then_body, depth);
            if let Some(eb) = else_body {
                out.push_str(" else");
                write_block(out, eb, depth);
            }
        }
        Stmt::ExprStmt(e) => write_expr(out, e, depth),
    }
}

fn write_expr(out: &mut String, e: &Expr, depth: usize) {
    write_expr_prec(out, e, 0, depth);
}

/// Outer precedence is the parent operator's precedence; if this expression's
/// own precedence is lower, it gets wrapped in parens. Atoms are highest.
fn write_expr_prec(out: &mut String, e: &Expr, outer: u8, depth: usize) {
    let prec = expr_precedence(e);
    let needs_parens = prec < outer;
    if needs_parens {
        out.push('(');
    }
    match e {
        Expr::Int(n) => write!(out, "{n}").unwrap(),
        Expr::Float(n) => {
            // Use `{}` so 1.0 renders as "1"? No — must keep decimal so it
            // re-lexes as Float, not Int. `{:?}` gives e.g. "1.0".
            write!(out, "{n:?}").unwrap();
        }
        Expr::Str(s) => {
            out.push('"');
            for c in s.chars() {
                match c {
                    '\\' => out.push_str("\\\\"),
                    '"' => out.push_str("\\\""),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    '\r' => out.push_str("\\r"),
                    other => out.push(other),
                }
            }
            out.push('"');
        }
        Expr::Ident(n) => out.push_str(n),
        Expr::Binary(op, l, r) => {
            // Pipe chains of 3+ stages render across lines for the
            // canonical lightsaber shape:
            //   matches = episodes
            //     |> filter(by: c)
            //     |> rank(by: c.weight)
            if matches!(op, BinOp::Pipe) {
                let chain = flatten_pipe_chain(e);
                if chain.len() >= 3 {
                    let p = binop_precedence(BinOp::Pipe);
                    write_expr_prec(out, chain[0], p, depth);
                    for seg in &chain[1..] {
                        out.push('\n');
                        write_indent(out, depth + 1);
                        out.push_str("|> ");
                        write_expr_prec(out, seg, p + 1, depth + 1);
                    }
                    if needs_parens {
                        out.push(')');
                    }
                    return;
                }
            }

            let p = binop_precedence(*op);
            // Left-assoc operators keep `l` at the same precedence; the right
            // operand needs `p+1` to force parens on equal-precedence siblings.
            // For `~>` (right-assoc), reverse the bias.
            let (lp, rp) = if matches!(op, BinOp::StateShift) {
                (p + 1, p)
            } else {
                (p, p + 1)
            };
            write_expr_prec(out, l, lp, depth);
            write!(out, " {} ", binop_str(*op)).unwrap();
            write_expr_prec(out, r, rp, depth);
        }
        Expr::Call { callee, args } => {
            out.push_str(callee);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                match a {
                    CallArg::Positional(e) => write_expr(out, e, depth),
                    CallArg::Named { name, value } => {
                        write!(out, "{name}: ").unwrap();
                        write_expr(out, value, depth);
                    }
                }
            }
            out.push(')');
        }
        Expr::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_expr(out, item, depth);
            }
            out.push(']');
        }
        Expr::FieldAccess { object, field } => {
            // Field access binds tighter than any binary; pass max precedence
            // to force parens around any binary on the LHS.
            write_expr_prec(out, object, u8::MAX, depth);
            write!(out, ".{field}").unwrap();
        }
        Expr::Match { scrutinee, arms } => {
            out.push_str("match ");
            write_expr(out, scrutinee, depth);
            out.push_str(" {\n");
            for arm in arms {
                write_indent(out, depth + 1);
                write_pattern(out, &arm.pattern);
                out.push_str(" -> ");
                write_arm_body(out, &arm.body, depth + 1);
                out.push('\n');
            }
            write_indent(out, depth);
            out.push('}');
        }
    }
    if needs_parens {
        out.push(')');
    }
}

fn write_arm_body(out: &mut String, body: &[Stmt], depth: usize) {
    // Single-statement bare-form (no braces) when there's one stmt and it's
    // an ExprStmt / Reply / Broadcast / Assign — anything except If or
    // nested-block scenarios. If form requires braces because it has its
    // own block.
    if body.len() == 1 && !matches!(body[0], Stmt::If { .. }) {
        // Render the single statement inline (no leading indent — the arm
        // already wrote its prefix).
        let mut inline = String::new();
        write_stmt(&mut inline, &body[0], 0);
        // Strip the leading indent that write_stmt always prepends.
        out.push_str(inline.trim_start_matches(INDENT));
        return;
    }
    // Multi-statement or contains If: brace block.
    out.push('{');
    if body.is_empty() {
        out.push('}');
        return;
    }
    out.push('\n');
    for s in body {
        write_stmt(out, s, depth + 1);
        out.push('\n');
    }
    write_indent(out, depth);
    out.push('}');
}

fn write_pattern(out: &mut String, p: &Pattern) {
    match p {
        Pattern::Wildcard => out.push('_'),
        Pattern::Constructor(path) => write_dotted(out, path),
    }
}

// --- Type expressions --------------------------------------------------

fn write_type(out: &mut String, t: &TypeExpr) {
    match t {
        TypeExpr::Path(segs) => write_dotted(out, segs),
        TypeExpr::List(inner) => {
            out.push('[');
            write_type(out, inner);
            out.push(']');
        }
        TypeExpr::Function { from, to, effects } => {
            // Function is right-associative; only wrap the `from` side if
            // it's itself a function.
            if matches!(**from, TypeExpr::Function { .. }) {
                out.push('(');
                write_type(out, from);
                out.push(')');
            } else {
                write_type(out, from);
            }
            out.push_str(" -> ");
            write_type(out, to);
            if !effects.is_empty() {
                out.push_str(" / {");
                for (i, eff) in effects.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    write_dotted(out, eff);
                }
                out.push('}');
            }
        }
        TypeExpr::Record(fields) => {
            out.push('{');
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write!(out, "{}: ", f.name).unwrap();
                write_type(out, &f.ty);
            }
            out.push('}');
        }
    }
}

// --- Helpers -----------------------------------------------------------

fn write_dotted(out: &mut String, segs: &[String]) {
    for (i, s) in segs.iter().enumerate() {
        if i > 0 {
            out.push('.');
        }
        out.push_str(s);
    }
}

/// Higher number = tighter binding. Atoms sit at `u8::MAX` effectively
/// (no parens ever).
fn expr_precedence(e: &Expr) -> u8 {
    match e {
        Expr::Binary(op, ..) => binop_precedence(*op),
        // Match is a brace-delimited expression; it doesn't need parens
        // when nested in operators because the braces already delimit it.
        _ => u8::MAX,
    }
}

/// Flatten a left-associative pipe chain into a `Vec` of segments, head first.
/// `a |> b |> c` parses as `Binary(Pipe, Binary(Pipe, a, b), c)`; this returns
/// `[a, b, c]`.
fn flatten_pipe_chain(e: &Expr) -> Vec<&Expr> {
    let mut segments = Vec::new();
    collect_pipe(e, &mut segments);
    segments
}

fn collect_pipe<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    match e {
        Expr::Binary(BinOp::Pipe, l, r) => {
            collect_pipe(l, out);
            out.push(r);
        }
        _ => out.push(e),
    }
}

fn binop_precedence(op: BinOp) -> u8 {
    match op {
        BinOp::StateShift => 1,
        BinOp::Pipe => 2,
        BinOp::Lt | BinOp::Gt | BinOp::Eq => 3,
        BinOp::Add | BinOp::Sub => 4,
        BinOp::Mul | BinOp::Div => 5,
    }
}

fn binop_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Eq => "==",
        BinOp::Pipe => "|>",
        BinOp::StateShift => "~>",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    /// Round-trip: parse, format, re-parse, format again — both formatted
    /// outputs must match. This is stronger than AST equality (which
    /// can't be used directly because AST nodes now carry spans that
    /// differ between original and reformatted source) and equivalent
    /// in practice — if the formatted strings agree, the parser
    /// interpreted both inputs as semantically the same program.
    fn round_trip(src: &str) {
        let m1 = parse(src).expect("first parse");
        let formatted1 = format(&m1);
        let m2 = parse(&formatted1)
            .unwrap_or_else(|errs| panic!("re-parse failed:\n{formatted1}\nerrs: {errs:?}"));
        let formatted2 = format(&m2);
        assert_eq!(formatted1, formatted2, "round-trip diverged");
    }

    /// Idempotency: format(format(x)) == format(x).
    fn idempotent(src: &str) {
        let m1 = parse(src).expect("parse");
        let f1 = format(&m1);
        let m2 = parse(&f1).expect("re-parse");
        let f2 = format(&m2);
        assert_eq!(f1, f2, "format is not idempotent.\nfirst:\n{f1}\nsecond:\n{f2}");
    }

    #[test]
    fn round_trips_empty_facet() {
        round_trip("facet X {}");
    }

    #[test]
    fn round_trips_facet_with_state() {
        round_trip("facet Hunger { /// _state level = 0 }");
    }

    #[test]
    fn round_trips_facet_with_state_and_handlers() {
        round_trip("facet X { /// _state traces = 0  /// _handlers on Cue c { reply traces } }");
    }

    #[test]
    fn round_trips_arithmetic_with_precedence() {
        round_trip("facet X { /// _handlers on T { x = 1 + 2 * 3 } }");
    }

    #[test]
    fn round_trips_state_shift_chain() {
        round_trip("facet X { /// _state x = 0  /// _handlers on T { x = x ~> x + 1 } }");
    }

    #[test]
    fn round_trips_pipe_chain() {
        round_trip("facet X { /// _handlers on T { x = a |> b() |> c(d) } }");
    }

    #[test]
    fn round_trips_named_args_and_field_access() {
        round_trip("facet X { /// _handlers on T { x = filter(by: c.weight) } }");
    }

    #[test]
    fn round_trips_lists() {
        round_trip("facet X { /// _state xs = []  /// _handlers on T { x = [1, 2, 3] } }");
    }

    #[test]
    fn round_trips_function_type_with_effects_in_state() {
        round_trip(
            "facet X { /// _state handler: Cue -> [Trace] / {io.sim.comms} = noop }",
        );
    }

    #[test]
    fn round_trips_match() {
        round_trip(
            "facet X { /// _handlers on S s { match s { Threat -> { x = 1 } _ -> {} } } }",
        );
    }

    #[test]
    fn round_trips_scheme() {
        round_trip(
            "scheme mind {
               /// _io
               clock: io.sim.clock
               siblings: io.sim.comms.peer
               /// _facets
               episodicMemory(clock, siblings)
               voice(clock)
               /// _dispatch_scripts
               on clock.tick sequence(episodicMemory -> voice)
             }",
        );
    }

    #[test]
    fn round_trips_scheme_with_parent() {
        round_trip(
            "scheme mind @ rubberDuck {
               /// _io
               clock: io.sim.clock
               /// _facets
               hunger(clock)
             }",
        );
    }

    #[test]
    fn formats_scheme_with_parent() {
        let m = parse("scheme mind @ rubberDuck {}").unwrap();
        let f = format(&m);
        assert!(f.contains("scheme mind @ rubberDuck"), "got:\n{f}");
    }

    #[test]
    fn formats_root_scheme_without_at_clause() {
        let m = parse("scheme mind {}").unwrap();
        let f = format(&m);
        assert!(!f.contains("@"), "root scheme shouldn't render `@`; got:\n{f}");
    }

    #[test]
    fn round_trips_module_with_facets_and_scheme() {
        round_trip(
            "facet Hunger { /// _state level = 0 }
             scheme mind { /// _io clock: io.sim.clock  /// _facets hunger(clock) }",
        );
    }

    #[test]
    fn idempotent_on_full_mind_example() {
        let src = std::fs::read_to_string("../../examples/agent.urchin").expect("read agent.urchin");
        idempotent(&src);
    }

    #[test]
    fn round_trips_full_mind_example() {
        let src = std::fs::read_to_string("../../examples/agent.urchin").expect("read agent.urchin");
        round_trip(&src);
    }

    // --- Spot tests for specific output shape ---

    #[test]
    fn formats_empty_facet_inline() {
        let m = parse("facet X {}").unwrap();
        assert_eq!(format(&m), "facet X {}\n");
    }

    #[test]
    fn formats_arithmetic_without_unnecessary_parens() {
        let m = parse("facet X { /// _handlers on T { x = 1 + 2 } }").unwrap();
        let f = format(&m);
        assert!(!f.contains("(1"), "no parens around `1` expected; got:\n{f}");
        assert!(f.contains("x = 1 + 2"));
    }

    #[test]
    fn single_pipe_stays_inline() {
        let m = parse("facet X { /// _handlers on T { x = a |> b() } }").unwrap();
        let f = format(&m);
        assert!(f.contains("x = a |> b()"), "expected inline pipe; got:\n{f}");
        assert!(!f.contains("\n      |>"), "single pipe should not wrap; got:\n{f}");
    }

    #[test]
    fn three_stage_pipe_wraps_across_lines() {
        let m = parse("facet X { /// _handlers on T { x = a |> b() |> c() } }").unwrap();
        let f = format(&m);
        // After "x = a", a newline + indent + "|>" should appear.
        assert!(
            f.contains("a\n      |> b()\n      |> c()"),
            "expected multi-line pipe; got:\n{f}"
        );
    }

    #[test]
    fn long_pipe_chain_in_assignment_round_trips() {
        round_trip("facet X { /// _handlers on T { matches = a |> b() |> c() |> d() |> e() } }");
    }

    #[test]
    fn pipe_wrap_is_idempotent() {
        idempotent("facet X { /// _handlers on T { x = a |> b() |> c() |> d() } }");
    }

    // --- IO declarations ---

    #[test]
    fn round_trips_empty_io_decl() {
        round_trip("io clock {}");
    }

    #[test]
    fn round_trips_io_decl_events_only() {
        round_trip(
            "io clock { /// _events tick(at: timestamp) }",
        );
    }

    #[test]
    fn round_trips_io_decl_requests_only() {
        round_trip(
            "io llm { /// _requests ask(prompt: string) -> string }",
        );
    }

    #[test]
    fn round_trips_io_decl_request_no_args() {
        round_trip(
            "io camera { /// _requests snapshot() -> image }",
        );
    }

    #[test]
    fn round_trips_io_decl_api_contracts() {
        round_trip(
            "io anthropic { /// _api_contracts request: {model: string, prompt: string} }",
        );
    }

    #[test]
    fn round_trips_io_decl_config() {
        round_trip("io clock { /// _config rate = 60  offset = 0 }");
    }

    #[test]
    fn round_trips_full_io_decl() {
        round_trip(
            "io anthropic {
               /// _requests
               ask(prompt: string) -> response

               /// _events
               rateLimited(retryAfter: int)

               /// _api_contracts
               response: {text: string, tokens: int}

               /// _config
               endpoint = \"https://api.anthropic.com\"
               retries = 3
             }",
        );
    }

    #[test]
    fn round_trips_module_with_facet_io_and_scheme() {
        round_trip(
            "facet Hunger { /// _state level = 0 }
             io clock { /// _events tick(at: timestamp) }
             scheme mind { /// _io c: io.sim.clock  /// _facets hunger(c) }",
        );
    }

    #[test]
    fn module_order_is_facets_then_io_then_schemes() {
        // Mixed declaration order in source — canonical output puts facets
        // first, io decls next, schemes last (small-to-large).
        let src = "scheme mind {}
                   io clock {}
                   facet X {}";
        let m = parse(src).unwrap();
        let f = format(&m);
        let facet_pos = f.find("facet X").expect("facet missing");
        let io_pos = f.find("io clock").expect("io missing");
        let scheme_pos = f.find("scheme mind").expect("scheme missing");
        assert!(facet_pos < io_pos && io_pos < scheme_pos, "got:\n{f}");
    }

    #[test]
    fn round_trips_record_type_in_state() {
        round_trip("facet X { /// _state shape: {x: int, y: int} = empty }");
    }
}
