/// Urchin AST — grows as the grammar grows.
///
/// Spans are deliberately omitted for now; they get added when error
/// messages need them. The shape here mirrors SPEC.md §3.

/// A `Module` holds the top-level declarations from one source file.
/// Roles and actors can appear in any order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Module {
    pub roles: Vec<RoleDecl>,
    pub actors: Vec<ActorDecl>,
}

/// An actor declaration. Body has three sections in canonical order:
/// IO spines first (the substrate), then role instances with their
/// IO + role-to-role wiring (who's plugged into what), then dispatch
/// declarations (how races resolve). No actor-level behavior code.
///
/// `parent` (optional) declares the actor's position in the topology
/// tree: `actor mind @ rubberDuck { ... }` reads as "the mind slot of
/// rubberDuck." The actor's name is the slot name in its parent.
/// Root actors (no parent) leave this `None`. Sibling and child
/// relationships are inferred from the union of all `@` clauses
/// across a project.
///
/// All identifiers — actor name and parent reference — are camelCase
/// per the language convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorDecl {
    pub name: String,
    pub parent: Option<String>,
    pub io_spines: Vec<IoSpine>,
    pub role_instances: Vec<RoleInstance>,
    pub dispatch: Vec<DispatchDecl>,
}

/// `name(io_arg, ...)(source -> method, ...)` — a composed role instance.
/// First parens lists the IO spines this instance can talk to. Second
/// parens (optional) wires interface methods this instance needs to
/// methods provided by sibling instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleInstance {
    pub name: String,
    pub io_args: Vec<String>,
    pub wires: Vec<RoleWire>,
}

/// `source -> method` — a wire from this instance's needed method to
/// the same-named method on the named source instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleWire {
    pub source: String,
    pub method: String,
}

/// `on spine.event <mode>` — dispatch keys on the spine-qualified event,
/// not the bare message type. Same message type from different spines can
/// dispatch differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchDecl {
    pub event: SpineEvent,
    pub mode: DispatchMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpineEvent {
    pub spine: String,
    pub event: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchMode {
    Parallel,
    Async,
    /// `sequence(a -> b -> c)` — instances fire in declared order. Names
    /// are role-instance names (lowercase camelCase), not type paths.
    Sequence(Vec<String>),
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
#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct StateField {
    pub name: String,
    pub ty: TypeExpr,
    /// Optional initializer: `level: float = 0.0`. Used by the runtime
    /// to seed the field when the role is instantiated. `None` means
    /// the typechecker will require an explicit init somewhere
    /// (potentially in a future role-level init block).
    pub init: Option<Expr>,
}

/// `on TypePath binding? { stmt* }`
#[derive(Debug, Clone, PartialEq)]
pub struct Handler {
    pub message_type: Vec<String>,
    pub binding: Option<String>,
    pub body: Vec<Stmt>,
}

/// Statements appear inside handler bodies.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `name = expr` — a local binding if `expr` has no `~>`, a state
    /// mutation if it does. Distinction left to the typechecker.
    Assign { name: String, value: Expr },
    /// `reply expr`
    Reply(Expr),
    /// `broadcast TypePath` or `broadcast TypePath(args)` — emit a message
    /// onto the actor's bus. `span` is the source range covering the whole
    /// `broadcast …` statement, used by the typechecker for diagnostics
    /// (e.g. composition-completeness errors point at the offending broadcast).
    Broadcast {
        message_type: Vec<String>,
        args: Vec<Expr>,
        span: std::ops::Range<usize>,
    },
    /// `if cond { then_body } else { else_body }`. Else is optional.
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    /// A bare expression statement.
    ExprStmt(Expr),
}

/// Only `PartialEq` is derivable (not `Eq`) because `Float` carries an `f64`.
/// Tests use `assert_eq!` on whole `Expr` values, which works through
/// `PartialEq`; downstream code that needs `Eq` should normalise floats first.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `name(arg, arg, ...)` — args can mix positional and `name: expr`.
    Call { callee: String, args: Vec<CallArg> },
    /// `[a, b, c]` — list literal. Empty list `[]` is allowed.
    List(Vec<Expr>),
    /// `expr.field` — field access. Left-associative, so `a.b.c` parses
    /// as `(a.b).c`.
    FieldAccess { object: Box<Expr>, field: String },
    /// `match scrutinee { Pat -> body  Pat -> body  ... }`
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    /// Body is always a `Vec<Stmt>`; a bare-expression arm body becomes
    /// a single `ExprStmt`. A block-bodied arm preserves its statements.
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// `_` — matches anything.
    Wildcard,
    /// A type path (`Threat`, `Food`, `io.sim.Tick`). For now, no
    /// destructuring of constructor args; that lands when sum types do.
    Constructor(Vec<String>),
}

/// A call argument is either positional (`filter(c)`) or named
/// (`filter(by: c)`). The compiler may require named args for some methods.
#[derive(Debug, Clone, PartialEq)]
pub enum CallArg {
    Positional(Expr),
    Named { name: String, value: Expr },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
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
