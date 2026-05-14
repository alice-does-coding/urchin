/// Urchin AST — grows as the grammar grows.
///
/// Spans are deliberately omitted for now; they get added when error
/// messages need them. The shape here mirrors SPEC.md §3.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub roles: Vec<RoleDecl>,
}

/// A role body has up to three sections in order: interface, state, handlers.
/// Per SPEC.md §3.1 each section is optional and identified by syntactic shape
/// (interface = bare `name:`, state = `~ name:`, handler = `on Type`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleDecl {
    pub name: String,
    pub interface: Vec<InterfaceMethod>,
    pub state: Vec<StateField>,
    pub handlers: Vec<Handler>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceMethod {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateField {
    pub name: String,
    pub ty: TypeExpr,
}

/// `on TypePath binding? { … }` — body is currently always empty;
/// expression grammar lands in a follow-up slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handler {
    pub message_type: Vec<String>,
    pub binding: Option<String>,
}

/// `Function` is right-associative — `A -> B -> C` parses as `A -> (B -> C)`.
/// Refinement types (`0..1`), generics (`[Trace]`), and effect annotations
/// (`/ {io.http}`) get added as the grammar grows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    Path(Vec<String>),
    Function(Box<TypeExpr>, Box<TypeExpr>),
}
