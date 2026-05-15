//! `urchin watch` — pretty-renders the NDJSON event stream emitted by
//! `urchin run`. Reads one JSON object per line from stdin; writes a
//! human-readable view to stdout.
//!
//! The five event types in the v1 schema, in roughly the order they
//! arrive:
//!
//!   - `actor_instantiated`  — topology, before any ticks
//!   - `role_instantiated`   — initial role state
//!   - `tick`                — section header for everything that follows
//!   - `state_assign`        — a `~>` swap, shown as `field: old → new`
//!   - `handler_return`      — the value a handler produced for a message
//!
//! Output is colored when stdout is a TTY and falls back to plain text
//! when piped. Unknown event shapes pass through as the original JSON so
//! `urchin watch` keeps working when the schema grows.

use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;

use serde_json::Value as Json;

pub fn run() -> Result<(), ExitCode> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let style = Style::new(stdout.is_terminal());
    let mut out = stdout.lock();
    let mut state = RenderState::default();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("urchin watch: stdin read error: {e}");
                return Err(ExitCode::from(2));
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        render_line(&mut out, &line, &style, &mut state);
    }
    Ok(())
}

#[derive(Default)]
struct RenderState {
    topology_open: bool,
    saw_first_tick: bool,
}

fn render_line(out: &mut dyn Write, line: &str, style: &Style, state: &mut RenderState) {
    let event: Json = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => {
            // Not JSON — pass through verbatim.
            let _ = writeln!(out, "{line}");
            return;
        }
    };
    let kind = event.get("event").and_then(|v| v.as_str()).unwrap_or("?");
    match kind {
        "actor_instantiated" => render_actor(out, &event, style, state),
        "role_instantiated" => render_role(out, &event, style, state),
        "tick" => render_tick(out, &event, style, state),
        "state_assign" => render_state_assign(out, &event, style),
        "handler_return" => render_handler_return(out, &event, style),
        _ => {
            let _ = writeln!(out, "{line}");
        }
    }
}

fn ensure_topology_header(out: &mut dyn Write, style: &Style, state: &mut RenderState) {
    if !state.topology_open {
        let _ = writeln!(out, "{}", style.section("topology"));
        state.topology_open = true;
    }
}

fn render_actor(out: &mut dyn Write, e: &Json, style: &Style, state: &mut RenderState) {
    ensure_topology_header(out, style, state);
    let actor = s(e, "actor");
    let parent = e.get("parent").and_then(|v| v.as_str());
    match parent {
        Some(p) => {
            let _ = writeln!(
                out,
                "  {} {} {} {}",
                style.dim("actor"),
                style.actor(actor),
                style.dim("@"),
                style.actor(p),
            );
        }
        None => {
            let _ = writeln!(out, "  {} {}", style.dim("actor"), style.actor(actor));
        }
    }
}

fn render_role(out: &mut dyn Write, e: &Json, style: &Style, state: &mut RenderState) {
    ensure_topology_header(out, style, state);
    let actor = s(e, "actor");
    let instance = s(e, "instance");
    let state_str = e
        .get("state")
        .and_then(|v| v.as_array())
        .map(|pairs| format_state_pairs(pairs, style))
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "    {} {}{}{} {}",
        style.dim("role"),
        style.actor(actor),
        style.dim("."),
        style.instance(instance),
        state_str,
    );
}

fn render_tick(out: &mut dyn Write, e: &Json, style: &Style, state: &mut RenderState) {
    let n = e.get("n").and_then(|v| v.as_u64()).unwrap_or(0);
    if !state.saw_first_tick {
        let _ = writeln!(out);
        state.saw_first_tick = true;
    } else {
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "{}", style.section(&format!("tick {n}")));
}

fn render_state_assign(out: &mut dyn Write, e: &Json, style: &Style) {
    let instance = s(e, "instance");
    let field = s(e, "field");
    let old_json = e.get("old");
    let new_json = e.get("new");
    let old = old_json.map(format_value).unwrap_or_default();
    let new = new_json.map(format_value).unwrap_or_default();

    // No-op swap (the handler ran an assign but value didn't change).
    // Render dimmed with `=` instead of `→` so the arrow always means
    // change. Keeps the event visible (it really happened) without
    // misleading the eye.
    let unchanged = old_json == new_json;
    if unchanged {
        let _ = writeln!(
            out,
            "  {}",
            style.dim(&format!("{instance} {field} = {new}")),
        );
        return;
    }

    let _ = writeln!(
        out,
        "  {} {} {} {} {} {}",
        style.instance(instance),
        style.field(&field),
        style.dim(":"),
        style.value(&old),
        style.arrow("→"),
        style.value_new(&new),
    );
}

fn render_handler_return(out: &mut dyn Write, e: &Json, style: &Style) {
    let instance = s(e, "instance");
    let message = s(e, "message");
    let value = e.get("value").map(format_value).unwrap_or_default();
    let _ = writeln!(
        out,
        "  {}{}{} {} {}",
        style.instance(instance),
        style.dim("."),
        style.message(&message),
        style.arrow("⇒"),
        style.value_return(&value),
    );
}

fn s<'a>(e: &'a Json, field: &str) -> &'a str {
    e.get(field).and_then(|v| v.as_str()).unwrap_or("?")
}

fn format_value(v: &Json) -> String {
    match v {
        Json::Null => "()".to_string(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.to_string(),
        Json::String(s) => format!("\"{s}\""),
        Json::Array(_) | Json::Object(_) => v.to_string(),
    }
}

fn format_state_pairs(pairs: &[Json], style: &Style) -> String {
    if pairs.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = pairs
        .iter()
        .filter_map(|p| {
            let arr = p.as_array()?;
            let key = arr.first()?.as_str()?;
            let val = arr.get(1).map(format_value).unwrap_or_default();
            Some(format!("{}{}{}", style.field(key), style.dim("="), style.value(&val)))
        })
        .collect();
    format!("{}{}{}", style.dim("{ "), parts.join(&style.dim(", ")), style.dim(" }"))
}

/// ANSI styling. No external dependency — the codes are short and the
/// surface is small. Switching to a crate later is a drop-in replacement.
struct Style {
    on: bool,
}

impl Style {
    fn new(on: bool) -> Self { Self { on } }

    fn wrap(&self, code: &str, s: &str) -> String {
        if self.on { format!("\x1b[{code}m{s}\x1b[0m") } else { s.to_string() }
    }

    fn dim(&self, s: &str) -> String { self.wrap("2", s) }
    fn section(&self, s: &str) -> String { self.wrap("1;36", &format!("── {s} ──")) }
    fn actor(&self, s: &str) -> String { self.wrap("36", s) }
    fn instance(&self, s: &str) -> String { self.wrap("35", s) }
    fn field(&self, s: &str) -> String { self.wrap("33", s) }
    fn message(&self, s: &str) -> String { self.wrap("33", s) }
    fn value(&self, s: &str) -> String { self.wrap("2", s) }
    fn value_new(&self, s: &str) -> String { self.wrap("32", s) }
    fn value_return(&self, s: &str) -> String { self.wrap("32", s) }
    fn arrow(&self, s: &str) -> String { self.wrap("2", s) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(lines: &[&str]) -> String {
        let style = Style::new(false);
        let mut out: Vec<u8> = Vec::new();
        let mut state = RenderState::default();
        for line in lines {
            render_line(&mut out, line, &style, &mut state);
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn renders_topology_then_tick() {
        let out = render(&[
            r#"{"event":"actor_instantiated","actor":"creativePersona","parent":"rubberDuck"}"#,
            r#"{"event":"role_instantiated","actor":"creativePersona","instance":"photographer","state":[["shotsTaken",0]]}"#,
            r#"{"event":"actor_instantiated","actor":"rubberDuck","parent":null}"#,
            r#"{"event":"tick","n":0}"#,
        ]);
        assert!(out.contains("── topology ──"));
        assert!(out.contains("actor creativePersona @ rubberDuck"));
        assert!(out.contains("actor rubberDuck"));
        assert!(out.contains("role creativePersona.photographer"));
        assert!(out.contains("shotsTaken=0"));
        assert!(out.contains("── tick 0 ──"));
    }

    #[test]
    fn renders_state_assign_with_old_new_arrow() {
        let out = render(&[r#"{"event":"state_assign","actor":"creativePersona","instance":"photographer","field":"shotsTaken","old":0,"new":1}"#]);
        assert!(out.contains("photographer"));
        assert!(out.contains("shotsTaken"));
        assert!(out.contains("0"));
        assert!(out.contains("→"));
        assert!(out.contains("1"));
    }

    #[test]
    fn renders_handler_return_with_arrow() {
        let out = render(&[r#"{"event":"handler_return","actor":"creativePersona","instance":"photographer","message":"tick","value":1}"#]);
        assert!(out.contains("photographer.tick"));
        assert!(out.contains("⇒"));
        assert!(out.contains("1"));
    }

    #[test]
    fn unknown_event_passes_through() {
        let out = render(&[r#"{"event":"some_future_event","fields":{"x":1}}"#]);
        assert!(out.contains("some_future_event"));
    }

    #[test]
    fn non_json_passes_through() {
        let out = render(&["not json at all"]);
        assert!(out.contains("not json at all"));
    }

    #[test]
    fn handler_return_with_unit_value() {
        let out = render(&[r#"{"event":"handler_return","actor":"a","instance":"i","message":"m","value":null}"#]);
        assert!(out.contains("()"));
    }

    #[test]
    fn noop_state_assign_renders_without_arrow() {
        // Same value old & new: the `if`-guarded re-fire pattern from
        // garden_arcade.urchin. Arrow should be absent so it never lies
        // about change.
        let out = render(&[r#"{"event":"state_assign","actor":"feedUser","instance":"poster","field":"isHot","old":1,"new":1}"#]);
        assert!(out.contains("isHot = 1"));
        assert!(!out.contains("→"));
    }

    #[test]
    fn changed_state_assign_still_uses_arrow() {
        let out = render(&[r#"{"event":"state_assign","actor":"feedUser","instance":"poster","field":"isHot","old":0,"new":1}"#]);
        assert!(out.contains("→"));
    }

    #[test]
    fn handler_return_uses_single_space_around_arrow() {
        let out = render(&[r#"{"event":"handler_return","actor":"a","instance":"poster","message":"tick","value":1}"#]);
        // No double spaces in the rendered line.
        assert!(!out.contains("  ⇒"), "unexpected double space before arrow: {out:?}");
    }

    #[test]
    fn string_value_is_quoted() {
        let out = render(&[r#"{"event":"handler_return","actor":"a","instance":"i","message":"m","value":"hello"}"#]);
        assert!(out.contains("\"hello\""));
    }
}
