# Urchin — Specification

_Living draft. Started 2026-05-13. Companion to `DIRECTION.md` (vision) and the implementation tree in `crates/`._

## Table of contents

0. [Preamble](#0-preamble)
1. Lexical structure _(TBD)_
2. Type system _(TBD)_
3. [Role grammar](#3-role-grammar) — **draft in progress**
4. Actor grammar _(TBD)_
5. Wire semantics _(TBD)_
6. Comms grammar _(TBD)_
7. Error & effect model _(TBD)_
8. Stdlib role taxonomy _(TBD)_
9. Worked examples _(TBD — the seed corpus)_
10. Open questions / deferred decisions

---

## 0. Preamble

### Scope

This document specifies the Urchin language: lexical structure, grammar, type system, semantics, and the inter-actor protocol (comms). It is the authoritative reference for the compiler implementation and for any human or AI writing Urchin code.

### Status

Living draft. Sections are added and revised in design sessions; each section is annotated with status. When the implementation in `crates/` diverges from this document, this document is the source of truth and the implementation needs to catch up.

### Not in scope

- Vision and design philosophy — see `DIRECTION.md`.
- Tutorial material — Section 9 (the seed corpus) is reference, not tutorial.
- Stdlib role _implementations_ — Section 8 specifies their interfaces only; implementations live in `crates/stdlib/`.

### Grammar conventions

Grammar is given in EBNF with the following conventions:

- `lowercase` — nonterminal
- `'literal'` — terminal
- `|` — alternation
- `?` — optional (zero or one)
- `*` — zero or more
- `+` — one or more
- `(...)` — grouping
- `// comment` — explanatory aside, not part of the grammar

Source is UTF-8. Indentation is two spaces. Newlines are LF.

---

## 3. Role grammar

_Status: first draft (2026-05-13). Open questions flagged inline at 3.9._

A **role** is the smallest compositional unit of Urchin. A role corresponds roughly to one cognitive faculty or one cohesive bundle of state-plus-behavior. Roles are assembled into actors (see Section 4); roles do not run on their own.

### 3.1 Structure of a role

A role declaration has up to four sections, in order:

1. **Header** — the `role` keyword, the role name, and (optionally) the parent role(s) it implements.
2. **Interface** — the methods this role exposes to the actor's wire layer.
3. **State** — the role's private state, prefixed with `~`.
4. **Handlers** — `on Message:` clauses that respond to incoming messages.

Sections are identified by their syntactic shape — not by section keywords — and appear at most once each, in the order above. Empty sections are simply omitted.

### 3.2 Grammar (EBNF)

```ebnf
role_decl         = 'role' role_name kinship? newline
                    interface_section?
                    state_section?
                    handler_section?

role_name         = pascal_ident                    // e.g. AssociativeMemory

kinship           = ':' role_path (',' role_path)*  // multi-kinship — see 3.9.A
role_path         = pascal_ident ('.' pascal_ident)*

interface_section = (interface_method newline)+
interface_method  = lower_ident ':' type_expr

state_section     = (state_field newline)+
state_field       = '~' lower_ident ':' type_expr

handler_section   = (handler newline)+
handler           = 'on' message_pattern ':' newline indented_block

message_pattern   = type_path bind_ident?           // e.g. `Query q` or `Tick`
type_path         = pascal_ident ('.' pascal_ident)*
bind_ident        = lower_ident
```

Lexical productions (`pascal_ident`, `lower_ident`, `newline`, `indented_block`) are defined in Section 1. The `type_expr` production is defined in Section 2 and includes function types (`A -> B`), parameterised types (`[T]`), and refinement types (`0..1`).

### 3.3 Canonical examples

```
role AssociativeMemory : Memory.Associative
  recall:  Query -> [Trace]
  store:   Trace -> ()
  ~ traces: [Trace]

  on Query q:
    matches = traces |> filter(by: q.cue) |> rank(by: q.weight)
    reply matches
```

```
role Hunger : Drive
  ~ level: 0..1

  on Tick:
    level = level ~> min(1.0, level + 0.01)
    if level > 0.7: broadcast Wants(Food)
```

### 3.4 Section-by-section semantics

**Header.** The `:` is the *kinship* relation. `role AssociativeMemory : Memory.Associative` declares that `AssociativeMemory` is a concrete role implementing the abstract role `Memory.Associative`. Multiple parents (mixin-style) are written `: A, B`. See 3.9.A.

**Interface.** Lines of the form `name: T` declare methods this role exposes. Function-typed methods appear as `name: ArgType -> ReturnType` because the function-type arrow lives inside `type_expr`. The wire layer (Section 5) connects external signals to these methods. Interface declarations are the *contract* a parent abstract role specifies and a concrete role must satisfy; restating a method on a concrete role re-declares it for clarity and local type-checking, and must agree with the parent's declaration.

**State.** Lines of the form `~ name: T` declare private state. State is mutable through the `~>` operator (3.5). State is sealed: other roles cannot read or write it directly — they can only observe state-derived effects through messages this role emits. The `~` prefix makes state declarations and mutations visually greppable.

**Handlers.** `on T b:` declares a handler that runs when a message of type `T` arrives on the actor's bus. The optional `b` binds the incoming message to a local name. Inside a handler the body has access to the bound message, the role's state fields (read/write via `~>`), and pure helper expressions. Handlers run to completion atomically within a role; concurrency exists at the actor and wire level, not within a role.

### 3.5 Body expressions in a handler

The following constructs appear in handler bodies. Full expression grammar is in Section 2.

- **Local binding**: `name = expr`. Locals are immutable after binding.
- **Pipe**: `expr |> fn(...)` is sugar for `fn(expr, ...)` — the piped value is the first argument.
- **State update**: `field = field ~> new_value`. The `~>` operator is *state shift*: it produces the next state value AND (when the result is assigned back to the same field) updates the field in place. See 3.9.B for why `~>` exists distinctly from `=`.
- **Reply**: `reply expr` sends `expr` back to the sender of the current message. Only valid inside a handler whose interface method declares a non-`()` return type.
- **Broadcast**: `broadcast Msg(...)` emits a message to the actor's bus. Any role wired to listen for `Msg` will receive it.
- **Conditional**: `if cond: expr` is a single-line conditional. Multi-line `if`/`else` and `match` are deferred to Section 2.

### 3.6 Naming rules (enforced by the compiler)

- Role names: **PascalCase** — `AssociativeMemory`, `Hunger`, `Voice`.
- Interface methods, state fields, handler binders, and locals: **snake_case** — `recall`, `traces`, `level`, `matches`.
- Predicate methods must begin with `is_`, `has_`, or `can_` (e.g. `is_satisfied`, `has_room`, `can_recall`).
- Methods or fields whose value is a timestamp must end in `_at` (`created_at`, `seen_at`).
- Identifiers referencing entities by handle must end in `_id` (`actor_id`, `trace_id`).

The compiler treats these as hard syntactic constraints — a misnamed item is a parse-time error, not a lint warning. The reasoning: naming carries type info, and the parser turning naming into free training signal for AI writing or reading Urchin code (per `DIRECTION.md`).

### 3.7 Tests adjacent to a role

A role may include one or more `test` blocks, syntactically a sibling section to handlers:

```
role Hunger : Drive
  ~ level: 0..1

  on Tick:
    level = level ~> min(1.0, level + 0.01)

  test "tick raises level by 0.01":
    given Hunger { level: 0.5 }
    send Tick
    expect level == 0.51
```

The `test` block is a first-class part of the role declaration — the spec lives next to the code. Full grammar TBD; see 3.9.C.

### 3.8 Typed comments

Three reserved comment forms are recognized by the compiler:

- `@invariant cond` — a condition the role must maintain across all handler executions. The compiler attempts to verify; un-verifiable invariants are reported but non-blocking.
- `@assumes cond` — a precondition the role assumes about its inputs or environment.
- `@because text` — machine-readable rationale for non-obvious code. Surfaced in error messages and LSP hover.

```
role Hunger : Drive
  @invariant level >= 0.0 and level <= 1.0
  ~ level: 0..1

  on Tick:
    @because "starvation cap prevents runaway growth"
    level = level ~> min(1.0, level + 0.01)
```

Free-form `//` comments — see 3.9.D.

### 3.9 Open questions

- **3.9.A — Multiple kinship.** Should a concrete role be allowed to implement multiple abstract roles (`role X : Memory.Associative, Recoverable`), or strictly one? Multi-kinship adds expressive power but complicates the type system (diamond inheritance, conflicting interface methods). **Drafted answer:** allow, but require explicit disambiguation when interface methods clash. Locking this affects Section 2 (kinship types).
- **3.9.B — Why `~>` distinct from `=`.** A state update could be written `level = level + 0.01`. Why introduce `~>`? Two candidate reasons: (1) `~>` makes mutation visually greppable, so every state change in a codebase can be found with one regex. (2) `~>` is the journal/replay hook point — the runtime (per `DIRECTION.md`) makes every state change reversible, and the operator marks where that machinery engages. **Drafted answer:** both. The visual distinction earns its keep at the language level; the journal hook earns its keep at the runtime level.
- **3.9.C — Test block syntax.** The sketch in 3.7 uses `given` / `send` / `expect`. Need to formalise: is `given` constructing a role instance or seeding state? Is `send` synchronous? How do tests assert on async outcomes? **Drafted answer:** defer until Section 5 (wire semantics) is drafted, since tests need to use the same mechanisms wires use.
- **3.9.D — Free-form comment policy.** Should `//` comments be entirely banned outside the typed forms (`@invariant`, `@assumes`, `@because`), or allowed-but-linted? **Drafted answer:** allowed but the LSP suggests converting to a typed form when the comment content looks load-bearing. Pure banning is purist; the linted middle path catches the cases that matter without blocking quick scratch notes.
- **3.9.E — Implicit-vs-explicit sections.** Should the four sections be marked with keywords (`interface:`, `state:`, `handlers:`) or left implicit (identified by syntactic shape, as drafted)? **Drafted answer:** implicit, per the DIRECTION examples. Explicit would be more discoverable but more ceremonious — runs against the "terse + regular" principle. Implicit relies on the syntactic distinguishers (`~` for state, `on` for handlers, bare `name:` for interface) being unambiguous, which they are if state always uses `~` and handlers always use `on`.

---

_End of section 3 draft. Section 4 (Actor grammar) is the natural next section — it composes roles via the `roles:`, `wire:`, and `speaks:` blocks._
