# Urchin — Specification

_Living draft. Started 2026-05-13. Companion to `DIRECTION.md` (vision) and the implementation tree in `crates/`._

## Table of contents

0. [Preamble](#0-preamble)
0.1. [Architecture](#01-architecture)
1. Lexical structure _(TBD — see `crates/parser/src/lexer.rs` for current token set)_
2. Type system _(TBD — see `crates/parser/src/ast.rs` for current `TypeExpr`)_
3. [Facet grammar](#3-facet-grammar) — **draft, in lockstep with the parser**
4. [Scheme grammar](#4-scheme-grammar) — **sketch**
5. Wire semantics _(folded into §4 / §6 — schemes compose facets, IO carries traffic; no separate wire layer)_
6. [IO grammar](#6-io-grammar) — **sketch**
7. Error & effect model _(TBD — algebraic effects with handlers, see §0.1)_
8. Stdlib facet taxonomy _(TBD)_
9. Worked examples _(TBD — the seed corpus; see `examples/agent.urchin` for the first)_
10. [Open questions / deferred decisions](#10-open-questions)

---

## 0. Preamble

### Scope

This document specifies the Urchin language: lexical structure, grammar, type system, semantics, and the IO layer through which schemes communicate. It is the authoritative reference for the compiler implementation and for any human or AI writing Urchin code.

### Status

Living draft. Sections are added and revised in design sessions; each section is annotated with status. When the implementation in `crates/` diverges from this document, this document is the source of truth and the implementation needs to catch up.

### Not in scope

- Vision and design philosophy — see `DIRECTION.md`.
- Tutorial material — Section 9 (the seed corpus) is reference, not tutorial.
- Stdlib facet _implementations_ — Section 8 specifies their interfaces only; implementations live in `crates/stdlib/`.

### Grammar conventions

Grammar is given in EBNF with the following conventions:

- `lowercase` — nonterminal
- `'literal'` — terminal
- `|` — alternation
- `?` — optional (zero or one)
- `*` — zero or more
- `+` — one or more
- `(...)` — grouping

Source is UTF-8. Newlines are LF. Block delimiters are `{` and `}`; whitespace within a block is not significant. Comments are `///` to end of line.

---

## 0.1 Architecture

Urchin is structured as **three layers, each locked to one paradigm**, with **events as the cross-cutting substrate**.

| Layer | What it is | Paradigm |
|---|---|---|
| **Facets** | Discrete, generic units of state + interface + handlers. No inter-facet relations of any kind. | Functional |
| **Schemes** | Bags of composed facets + declared IO spines. The unit with relational mappings (parent/sibling/child topology). All algorithm is emergent from the facet mix; no scheme-level code. | Kay-original OOP (encapsulation + identity + message-passing, no inheritance) |
| **IO** | Every operation that crosses an scheme's boundary, including agent-to-agent comms. Namespaced as `io.{kind}.*`; tracked as a typed effect with full algebraic-effect handlers. | Telecom (one IO flavor among many) generalized |

### Facets do not relate to each other

A facet has no parent, no kinship, no inheritance, and no abstract/concrete distinction. Facets are flat building blocks. Composition lives at the scheme level. Cross-cutting capabilities (recoverability, observability, etc.) are expressed by composing capability facets into the scheme that needs them, not by inheriting capability into a base facet.

### Schemes are minimal

An scheme declaration carries only:

1. The facets it composes,
2. The IO spines it declares,
3. Dispatch declarations when 2+ composed facets handle the same message type.

There is no scheme-level behavior code. Behavior is what the facet mix emergently does when IO arrives. Variation between agents in a simulation is variation in facet composition.

### IO is the only boundary

There is no separate "wire" or "bus" mechanism for inter-scheme communication. Scheme-to-scheme traffic is just IO (typically `io.sim.comms.*`), one IO flavor among many.

Namespace shape:

- `io.sim.{comms,spatial,clock,random,...}.*` — simulation-internal, deterministic, replayable, total (no `Result` wrapping)
- `io.{http,ws,sms,fs,db,...}.*` — real-world, returns `Result`, can fail/timeout

### Sim is deterministic and replayable

Every simulation has a journaled seed. PRNG draws (`io.sim.random.*`) regenerate from the seed; non-sim IO inputs are journaled. Replay is "same seed + same journaled real-world inputs → bit-identical state." Time-travel debugging falls out for free.

### IO is tracked as an effect

Facets and handlers carry an inferred effect set. A handler that only mutates state has an empty effect set; one that calls `io.sim.comms.send` carries `{io.sim.comms}`; one that calls `io.http.get` carries `{io.http}`. Effects show up in signatures (currently sketched as `T -> U / {effects}`).

Urchin uses **full algebraic effects with handlers** (Koka / Frank / Eff lineage). Any scheme or test harness can wrap a sub-region and intercept its effects: mocking is "wrap with a handler that fakes the IO"; sandboxing is "wrap with a handler that rejects forbidden effects." Time-travel debugging via effect replay falls out naturally.

### Multi-handler dispatch

When 2+ composed facets handle the same message type, the scheme MUST specify how they fire — no implicit default. Three modes:

- `on T parallel` — all handlers fire simultaneously; sealed-state means no races; broadcasts queue for the next step.
- `on T sequence(A -> B -> C)` — handlers fire in declared order; later handlers see earlier ones' state changes and broadcasts.
- `on T async` — fire-and-forget, no wait. Deterministic schedule under the hood for replay.

When 0 or 1 composed facets handle a type, no dispatch declaration needed. **0 composed handlers for a possible incoming type is a compile-time error.**

---

## 3. Facet grammar

_Status: draft, in lockstep with `crates/parser`. Sections marked **(parsed)** are accepted by the current parser; sections marked **(planned)** are designed but not yet implemented._

A **facet** is the smallest compositional unit of Urchin. A facet corresponds roughly to one cognitive faculty or one cohesive bundle of state-plus-behavior. Facets are assembled into schemes (see Section 4); facets do not run on their own. **Facets do not relate to each other** — see §0.1.

### 3.1 Structure of a facet

A facet declaration has up to three sections inside its brace-delimited body, in order:

1. **Interface** — the methods this facet exposes to the scheme's IO layer.
2. **State** — the facet's private state, prefixed with `~`.
3. **Handlers** — `on Message { … }` clauses that respond to incoming messages.

Sections are identified by their syntactic shape (interface = bare `name:`, state = `~ name:`, handler = `on TypePath`) and appear at most once each, in the order above. **Order is enforced** — a state field after a handler is a parse error, not a reorder hint. Empty sections are simply omitted.

### 3.2 Grammar (EBNF) **(parsed)**

```ebnf
facet_decl         = 'facet' facet_name '{'
                      interface_method*
                      state_field*
                      handler*
                    '}'

facet_name         = pascal_ident                    // e.g. EpisodicMemory

interface_method  = lower_ident ':' type_expr
state_field       = '~' lower_ident ':' type_expr
handler           = 'on' type_path lower_ident? '{' stmt* '}'

type_expr         = type_atom ('->' type_expr)?     // right-associative
type_atom         = type_path
type_path         = pascal_ident ('.' pascal_ident)*

stmt              = lower_ident '=' expr            // local binding or state mutation
                  | 'reply' expr
                  | expr

expr              = expr '~>' expr                  // right-associative, lowest precedence
                  | expr '|>' expr                  // left-associative
                  | expr '+' expr                   // left-associative
                  | call
                  | '(' expr ')'
                  | int_literal
                  | lower_ident

call              = lower_ident '(' (expr (',' expr)*)? ')'
```

**Operator precedence** (low → high): `~>` < `|>` < `+` < call/atom.
- `~>` is **right-associative** so `a ~> b ~> c` parses as `a ~> (b ~> c)` — chained state shifts compose.
- `|>` and `+` are **left-associative** so `a |> b |> c` reads left-to-right (the lightsaber).

### 3.3 Canonical example **(parsed)**

```ur
/// from examples/agent.urchin
facet EpisodicMemory {
  record: Event -> Unit
  recall: Cue -> int

  ~ count: int

  on Event e {
    count = count ~> count + 1
  }

  on Cue c {
    reply count
  }
}
```

### 3.4 Section-by-section semantics

**Interface methods.** `name: T` declares a method this facet exposes. Function-typed methods appear as `name: Arg -> Ret`. The IO layer connects external signals to these methods. There is no separate "implements" relation — a facet just exposes what it exposes; an scheme composing it may or may not wire the methods through.

**State fields.** `~ name: T` declares private state. State is sealed: other facets cannot read or write it directly. The `~` prefix makes state declarations and mutations greppable in one regex AND marks the journal hook point — every `~>` mutation is where the runtime engages reversibility machinery. (Resolved §3.9.B: both reasons — language-level greppability and runtime-level journal hook.)

**Handlers.** `on T b { … }` declares a handler that runs when a message of type `T` arrives at the scheme and dispatch resolves to this facet. The optional `b` binds the message to a local name. Inside a handler the body has access to the bound message, the facet's state fields (read normally, mutate via `~>`), and pure helper expressions.

### 3.5 Handler-body statements **(parsed)**

- **Assignment**: `name = expr`. Local binding if `expr` contains no `~>`; state mutation if it does. The typechecker classifies based on whether `name` refers to sealed state.
- **Reply**: `reply expr` sends `expr` back to the caller of the current message. Valid only inside a handler whose interface method declares a non-`Unit` return type.
- **Expression statement**: a bare expression. Mostly used for pipe chains exiting via a side-effect verb.

### 3.6 Naming rules **(planned — not yet enforced by the compiler)**

- **All identifiers are camelCase** — facet names, scheme names, message types, constructor patterns, broadcast tags, IO spine names, facet instance names, methods, fields, handler bindings, locals. PascalCase does not appear in idiomatic Urchin.
- Single-word identifiers are simply lowercase: `hunger`, `voice`, `tick`, `cue`, `episodes`, `mood`, `calm`, `food`.
- Multi-word identifiers join words with internal capitals: `episodicMemory`, `lastSnapshotAt`, `schemeId`, `isSatisfied`.
- Predicate methods must begin with `is`, `has`, or `can` (e.g. `isSatisfied`, `hasRoom`, `canRecall`).
- Methods or fields whose value is a timestamp must end in `At` (`createdAt`, `seenAt`).
- Identifiers referencing entities by handle must end in `Id` (`schemeId`, `traceId`).

The compiler will treat these as hard syntactic constraints, not lint warnings. Naming carries information (predicates / timestamps / handles), and naming-as-rule turns conventions into free training signal for AI writing or reading Urchin code. The reader infers what kind of thing a name refers to from syntactic position — `facet hunger {`, `on tick {`, `match s { calm -> ... }` — not from casing.

### 3.7 Tests adjacent to a facet **(planned)**

A facet may include `test` blocks as a sibling section to handlers. Full grammar deferred to §5 (wire semantics) since tests reuse the same dispatch mechanisms.

### 3.8 Typed comments **(planned)**

Three reserved doc-comment forms recognized by the compiler:

- `@invariant cond` — a condition the facet maintains across all handler executions.
- `@assumes cond` — a precondition about inputs or environment.
- `@because text` — machine-readable rationale for non-obvious code; surfaced in error messages and LSP hover.

### 3.9 Open questions

- **3.9.A — Multiple kinship.** **RESOLVED — dissolved.** Facets do not relate to each other (see §0.1). Cross-cutting capabilities are composed at the scheme level, not inherited at the facet level. The original investigation in `design/multi-kinship.md` is retired.
- **3.9.B — Why `~>` distinct from `=`.** **RESOLVED — both reasons.** Greppability at language level + journal hook at runtime level. See §3.4.
- **3.9.C — Test block syntax.** Open. Defer until handler/IO semantics are stable.
- **3.9.D — Free-form comment policy.** **RESOLVED — only `///`.** Single-line `///` and multi-line bracketed by `///` markers. No other comment form.
- **3.9.E — Implicit vs explicit section keywords.** **RESOLVED — implicit.** Sections identified by syntactic shape (`~`, `on`, bare `name:`); no `interface:`, `state:`, `handlers:` keywords.

---

## 4. Scheme grammar

_Status: sketch, ahead of the parser._

An scheme declaration is intentionally minimal: composed facets, declared IO spines, and (when needed) dispatch declarations. There is no scheme-level behavior code.

### 4.1 Sketched grammar

```ebnf
scheme_decl   = 'scheme' scheme_name '{'
                 facet_compose*           // bare PascalCase paths
                 dispatch_decl*          // only when 2+ facets handle the same type
                 io_spine*               // name : io.<path>
               '}'

facet_compose = type_path                 // e.g. Memory.Associative
dispatch_decl = 'on' type_path dispatch_mode
dispatch_mode = 'parallel' | 'async' | 'sequence' '(' type_path ('->' type_path)+ ')'
io_spine     = lower_ident ':' io_path
io_path      = 'io' '.' lower_ident ('.' lower_ident)*
```

### 4.2 Sketched example

```ur
scheme Mind {
  Memory.Associative
  Hunger
  Voice
  NegativeBias

  on Stimulus parallel
  on Tick     sequence(Voice -> NegativeBias)

  http:     io.http.server
  clock:    io.sim.clock
  siblings: io.sim.comms.peer
}
```

### 4.3 Composition completeness rule

Compile-time error if any message type the scheme can possibly receive — from a declared IO spine OR from a `broadcast` in any composed facet — has no composed-facet handler.

---

## 6. IO grammar

_Status: sketch, ahead of the parser._

The IO layer is the only boundary substrate. Every IO module exposes typed templates with the same shape, parameterized by direction, content type, and (for sim) edge-kind / (for real) failure mode.

### 6.1 Namespace

- `io.sim.*` — simulation-internal. Deterministic, replayable, total (no `Result`). Examples: `io.sim.comms`, `io.sim.spatial`, `io.sim.clock`, `io.sim.random`.
- `io.{http,ws,sms,fs,db,audio,...}.*` — real-world. Returns `Result`, can fail, time out.

### 6.2 Effect signatures (sketched)

```ur
fetch: Url -> Result[Json, FetchError] / {io.http}
```

The `/ {effects}` clause is the effect set. Most signatures get effects inferred; explicit annotation is for readability and contract-strengthening. The effect-set delimiter (`{}`, `[]`, or `<>`) is open — `{}` collides visually with `///` doc comments but parses unambiguously.

### 6.3 Algebraic effects with handlers

A handler can wrap a region of code and intercept all effect operations performed inside it. Test harnesses use this for mocking; runtime journaling uses this for replay. Detailed semantics in §7.

---

## 10. Open questions

Decisions still on the table:

- **Effect-set delimiter** — `{}` vs `[]` vs `<>`. Currently `{}` in sketches; reuses brace glyph but parses unambiguously.
- **Spatial substrate** — whether scheme coordinates are in the language semantics or only in the IDE projection. Three flavors named (substrate / view / hybrid-emergent); not yet locked.
- **Performative layer** — whether KQML/FIPA-style performatives (`tell`, `ask`, `subscribe`) are baked into IO templates or are just a vocabulary of template parameters.
- **Newline significance** — currently the lexer treats all whitespace as insignificant; multi-line expression chains (`traces |>\n  filter(by: c) |>\n  reply`) need either lexer-tracked newlines or a smarter expression parser.
- **Visibility** — likely no separate modifier needed (state is sealed by `~`, interface methods are public, locals are local). Confirmed only when we hit a case that needs it.

---

_End of spec snapshot. Sections marked **(parsed)** track the implementation; sketched sections (§4, §6) get hardened as their parsers land._
