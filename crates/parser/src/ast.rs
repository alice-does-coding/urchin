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

/// `on TypePath binding? { stmt* }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handler {
    pub message_type: Vec<String>,
    pub binding: Option<String>,
    pub body: Vec<Stmt>,
}

/// Statements appear inside handler bodies. The set is intentionally small
/// for this slice — assignment (local binding or state mutation, distinguished
/// by whether the RHS contains a `~>`), `reply expr`, or a bare expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `name = expr` — a local binding if `expr` has no `~>`, a state
    /// mutation if it does. Distinction left to the typechecker.
    Assign { name: String, value: Expr },
    /// `reply expr`
    Reply(Expr),
    /// A bare expression (mostly for pipe chains that exit via `reply`
    /// or `broadcast` — neither of which is a statement).
    ExprStmt(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i64),
    Ident(String),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `name(arg, arg, ...)`
    Call { callee: String, args: Vec<Expr> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `|>` — pipe (left-associative, low precedence)
    Pipe,
    /// `~>` — state shift (right-associative, lowest precedence)
    StateShift,
}

/// `Function` is right-associative — `A -> B -> C` parses as `A -> (B -> C)`.
/// Refinement types (`0..1`), generics (`[Trace]`), and effect annotations
/// (`/ {io.http}`) get added as the grammar grows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    Path(Vec<String>),
    Function(Box<TypeExpr>, Box<TypeExpr>),
}
