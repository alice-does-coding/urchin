/// Urchin AST — grows as the grammar grows.
///
/// Spans are deliberately omitted for now; they get added when error
/// messages need them. The shape here mirrors SPEC.md §3.

/// A `Module` holds the top-level declarations from one source file.
/// Facets, schemes, and io declarations can appear in any order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Module {
    pub facets: Vec<FacetDecl>,
    pub schemes: Vec<SchemeDecl>,
    pub io_decls: Vec<IoDecl>,
}

/// `io <name> { _interface ... _api_contracts ... _connection_handlers ... }`
///
/// Declares an io module — the typed contract between schemes and an
/// underlying API/SDK/network thing. Pure declarative; the runtime
/// synthesizes the implementation from interface + api_contracts +
/// connection config.
///
/// Three sections, parallel to scheme/facet section taxonomy:
///   - `_interface`           — what schemes see (event/method entries)
///   - `_api_contracts`       — wire-format record types (internal)
///   - `_connection_handlers` — config values for the connection
#[derive(Debug, Clone, PartialEq)]
pub struct IoDecl {
    pub name: String,
    pub interface: Vec<IoInterfaceEntry>,
    pub api_contracts: Vec<TypeAlias>,
    pub connection_handlers: Vec<ConnectionHandler>,
}

/// One entry in an io's `_interface` section.
#[derive(Debug, Clone, PartialEq)]
pub enum IoInterfaceEntry {
    /// `event name: ResultType` — the io produces this; schemes handle via `on`.
    Event { name: String, ty: TypeExpr },
    /// `method name: ArgType -> ResultType` — schemes call this.
    Method { name: String, ty: TypeExpr },
}

/// `name: TypeExpr` in `_api_contracts` — declares `name` as an alias
/// for `TypeExpr` (typically a record type). Internal to the io module.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAlias {
    pub name: String,
    pub ty: TypeExpr,
}

/// `name = init` in `_connection_handlers` — config value the runtime
/// uses to set up the underlying connection.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionHandler {
    pub name: String,
    pub init: Expr,
}

/// An scheme declaration. Body has three sections in canonical order:
/// IO spines first (the substrate), then facet instances with their
/// IO + facet-to-facet wiring (who's plugged into what), then dispatch
/// declarations (how races resolve). No scheme-level behavior code.
///
/// `parent` (optional) declares the scheme's position in the topology
/// tree: `scheme mind @ rubberDuck { ... }` reads as "the mind slot of
/// rubberDuck." The scheme's name is the slot name in its parent.
/// Root schemes (no parent) leave this `None`. Sibling and child
/// relationships are inferred from the union of all `@` clauses
/// across a project.
///
/// All identifiers — scheme name and parent reference — are camelCase
/// per the language convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemeDecl {
    pub name: String,
    pub parent: Option<String>,
    pub io_spines: Vec<IoSpine>,
    pub facet_instances: Vec<FacetInstance>,
    pub dispatch: Vec<DispatchDecl>,
}

/// `name(io_arg, ...)` — a composed facet instance. Parens lists the IO
/// spines this instance can talk to. Cross-facet coordination happens
/// through broadcasts and message handlers, not through method-wire
/// bindings — there are no interface methods to bind anymore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetInstance {
    pub name: String,
    pub io_args: Vec<String>,
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
    /// are facet-instance names (lowercase camelCase), not type paths.
    Sequence(Vec<String>),
}

/// `name: io.<path>` — a typed channel into the world (sim or real).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoSpine {
    pub name: String,
    pub io_path: Vec<String>,
}

/// A facet body has two sections: state, handlers. Each section is optional
/// and identified by its `/// _state` / `/// _handlers` marker (or, for
/// legacy code, by the leading `~` on state fields and `on` on handlers).
#[derive(Debug, Clone, PartialEq)]
pub struct FacetDecl {
    pub name: String,
    pub state: Vec<StateField>,
    pub handlers: Vec<Handler>,
}

/// State field — `name = init` (canonical) or `name: type = init` (legacy).
/// Init is required: every state field must declare its initial value, and
/// the type can be inferred from that value. Type annotation is optional.
#[derive(Debug, Clone, PartialEq)]
pub struct StateField {
    pub name: String,
    /// Optional type annotation. When present, the runtime checks the init
    /// value matches; when absent, the type is inferred from the init.
    pub ty: Option<TypeExpr>,
    /// REQUIRED initializer. Default-required-and-inferred is the model;
    /// no state field can be declared without an init value.
    pub init: Expr,
}

/// `on TypePath binding? ( -> ReturnTy )? { stmt* }`
///
/// Handler bodies are **block expressions**: the trailing `Stmt::ExprStmt`,
/// if present, is the handler's return value. If the body's last stmt is
/// not an `ExprStmt`, the handler returns unit.
///
/// `return_ty` is the optional declared return type. When `None`, the
/// type is inferred from the body's trailing expression (or unit if
/// none). When `Some`, the typechecker enforces that the body's value
/// matches.
#[derive(Debug, Clone, PartialEq)]
pub struct Handler {
    pub message_type: Vec<String>,
    pub binding: Option<String>,
    pub return_ty: Option<TypeExpr>,
    pub body: Vec<Stmt>,
}

/// Statements appear inside handler bodies.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `name = expr` — a local binding if `expr` has no `~>`, a state
    /// mutation if it does. Distinction left to the typechecker.
    Assign { name: String, value: Expr },
    /// `if cond { then_body } else { else_body }`. Else is optional.
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    /// A bare expression statement. Becomes the implicit return value
    /// when in tail position of a handler body.
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
    /// `{name: type, ...}` — record type with named fields.
    Record(Vec<RecordField>),
}

/// One field in a record type: `name: TypeExpr`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordField {
    pub name: String,
    pub ty: TypeExpr,
}
