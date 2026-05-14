//! Execution environment for a single handler invocation.
//!
//! Two layers:
//!   - `RoleState` — persistent state fields owned by a role instance.
//!     Survives across handler invocations. `~>` writes to this layer.
//!   - `Env` — transient local bindings made by `name = expr` statements
//!     inside a handler body. Lives only for the duration of one handler
//!     invocation; gone when the handler returns.
//!
//! The split mirrors the grammar's two-flavor `=`: a plain assign to a
//! state-field name updates `RoleState` via `~>`; a plain assign to a
//! fresh name updates `Env`. The interpreter resolves which layer to hit
//! based on whether the LHS is a known state field.

use std::collections::HashMap;

use crate::value::Value;

#[derive(Debug, Clone, Default)]
pub struct RoleState {
    fields: HashMap<String, Value>,
}

impl RoleState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: &str, value: Value) {
        self.fields.insert(name.to_string(), value);
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.fields.contains_key(name)
    }

    pub fn fields(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.fields.iter()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Env {
    bindings: HashMap<String, Value>,
}

impl Env {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: &str, value: Value) {
        self.bindings.insert(name.to_string(), value);
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings.get(name)
    }
}
