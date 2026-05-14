/// Urchin AST — minimal first slice. Grows as the grammar grows.
///
/// Spans are deliberately omitted for now; they get added when error
/// messages need them. The shape here mirrors SPEC.md §3.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub roles: Vec<RoleDecl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleDecl {
    pub name: String,
    pub state: Vec<StateField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateField {
    pub name: String,
    pub ty: TypeExpr,
}

/// TypeExpr is intentionally minimal — for the first parse slice it only
/// holds dotted paths (`int`, `Trace`, `Memory.Associative`). Refinement
/// types (`0..1`), generics (`[Trace]`), function types (`A -> B`), and
/// effect annotations (`/ {io.http}`) get added as the grammar grows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    Path(Vec<String>),
}
