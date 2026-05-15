//! Structured event log — JSON Lines to stdout.
//!
//! Every interesting moment in a run emits one line of JSON. The five
//! event types cover the v1 milestone:
//!   - `scheme_instantiated`
//!   - `facet_instantiated`
//!   - `tick`
//!   - `state_assign` (a `~>` swap; the journal hook even pre-journal)
//!   - `handler_return`
//!
//! Observers consume the stream from stdout; downstream tooling can
//! filter, pretty-print, or replay from it.

use serde::Serialize;

use crate::value::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    SchemeInstantiated {
        scheme: String,
        parent: Option<String>,
    },
    FacetInstantiated {
        scheme: String,
        instance: String,
        state: Vec<(String, Value)>,
    },
    Tick {
        n: u64,
    },
    StateAssign {
        scheme: String,
        instance: String,
        field: String,
        old: Value,
        new: Value,
    },
    HandlerReturn {
        scheme: String,
        instance: String,
        message: String,
        value: Value,
    },
}

/// Sink for events — abstracts stdout so tests can capture into a Vec.
pub trait EventSink {
    fn emit(&mut self, event: Event);
}

/// Writes one JSON object per line to stdout.
pub struct StdoutSink;

impl EventSink for StdoutSink {
    fn emit(&mut self, event: Event) {
        let line = serde_json::to_string(&event).expect("event serializes");
        println!("{line}");
    }
}

/// Captures events for assertions in tests.
#[derive(Default)]
pub struct VecSink {
    pub events: Vec<Event>,
}

impl EventSink for VecSink {
    fn emit(&mut self, event: Event) {
        self.events.push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_serializes_with_event_tag() {
        let e = Event::Tick { n: 3 };
        let s = serde_json::to_string(&e).unwrap();
        assert_eq!(s, r#"{"event":"tick","n":3}"#);
    }

    #[test]
    fn state_assign_serializes_with_snake_case_tag() {
        let e = Event::StateAssign {
            scheme: "creativePersona".into(),
            instance: "photographer".into(),
            field: "shotsTaken".into(),
            old: Value::Int(0),
            new: Value::Int(1),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains(r#""event":"state_assign""#));
        assert!(s.contains(r#""field":"shotsTaken""#));
        assert!(s.contains(r#""old":0"#));
        assert!(s.contains(r#""new":1"#));
    }

    #[test]
    fn vec_sink_collects() {
        let mut sink = VecSink::default();
        sink.emit(Event::Tick { n: 0 });
        sink.emit(Event::Tick { n: 1 });
        assert_eq!(sink.events.len(), 2);
    }
}
