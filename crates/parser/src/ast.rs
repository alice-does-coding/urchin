/// Urchin AST — grows as the grammar grows.
///
/// Spans are deliberately omitted for now; they get added when error
/// messages need them. The shape here mirrors SPEC.md §3.

/// A `Module` holds the top-level declarations from one source file.
/// Roles and actors can appear in any order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Module {
    pub roles: Vec<RoleDecl>,
    pub actors: Vec<ActorDecl>,
}

/// An actor is minimal per SPEC.md §0.1: composed roles, optional dispatch
/// declarations (when 2+ composed roles handle the same message type), and
/// declared IO spines. No actor-level behavior code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorDecl {
    pub name: String,
    pub composed_roles: Vec<Vec<String>>,
    pub dispatch: Vec<DispatchDecl>,
    pub io_spines: Vec<IoSpine>,
}

/// `on TypePath <mode>` — how to fire when 2+ composed roles handle the same type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchDecl {
    pub message_type: Vec<String>,
    pub mode: DispatchMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchMode {
    Parallel,
    Async,
    /// `sequence(A -> B -> C)` — handlers fire in declared order.
    Sequence(Vec<Vec<String>>),
}

/// `name: io.<path>` — a typed channel into the world (sim or real).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoSpine {
    pub name: String,
    pub io_path: Vec<String>,
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

/// Statements appear inside handler bodies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `name = expr` — a local binding if `expr` has no `~>`, a state
    /// mutation if it does. Distinction left to the typechecker.
    Assign { name: String, value: Expr },
    /// `reply expr`
    Reply(Expr),
    /// `broadcast TypePath` or `broadcast TypePath(args)` — emit a message
    /// onto the actor's bus.
    Broadcast { message_type: Vec<String>, args: Vec<Expr> },
    /// `if cond { then_body } else { else_body }`. Else is optional.
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    /// A bare expression statement.
    ExprStmt(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i64),
    Ident(String),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `name(arg, arg, ...)` — args can mix positional and `name: expr`.
    Call { callee: String, args: Vec<CallArg> },
    /// `[a, b, c]` — list literal. Empty list `[]` is allowed.
    List(Vec<Expr>),
    /// `expr.field` — field access. Left-associative, so `a.b.c` parses
    /// as `(a.b).c`.
    FieldAccess { object: Box<Expr>, field: String },
}

/// A call argument is either positional (`filter(c)`) or named
/// (`filter(by: c)`). The compiler may require named args for some methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallArg {
    Positional(Expr),
    Named { name: String, value: Expr },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `==`
    Eq,
    /// `|>` — pipe (left-associative, low precedence)
    Pipe,
    /// `~>` — state shift (right-associative, lowest precedence)
    StateShift,
}

/// `Function` is right-associative — `A -> B -> C` parses as `A -> (B -> C)`.
/// `effects` carries the optional algebraic-effect set written as
/// `/ {io.path, io.path}` after the arrow's RHS; an empty vector means
/// the function is effect-pure (with respect to the IO effect lattice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    Path(Vec<String>),
    Function {
        from: Box<TypeExpr>,
        to: Box<TypeExpr>,
        effects: Vec<Vec<String>>,
    },
    /// `[T]` — homogeneous list type.
    List(Box<TypeExpr>),
}
