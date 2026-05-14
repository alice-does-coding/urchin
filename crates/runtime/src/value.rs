//! Runtime values. Mirrors what handler expressions can produce.
//!
//! `Unit` is the value of a handler whose body has no trailing expression
//! (or one whose trailing statement is not an `ExprStmt`). `Record` is
//! present for forward-compatibility with the api-contract record types
//! shipped in slice 39; the interpreter doesn't construct records yet,
//! but the variant exists so io-call return values can carry them.
//!
//! Only `PartialEq` (not `Eq`) because `Float(f64)` precludes total eq.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Record(Vec<(String, Value)>),
    /// The value of a handler with no tail expression. Serializes to JSON null.
    #[serde(serialize_with = "serialize_unit")]
    Unit,
}

fn serialize_unit<S: serde::Serializer>(s: S) -> Result<S::Ok, S::Error> {
    s.serialize_unit()
}

impl Value {
    /// Convenience for tests / pattern guards.
    pub fn as_int(&self) -> Option<i64> {
        if let Value::Int(n) = self { Some(*n) } else { None }
    }
}
