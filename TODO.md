# TODO

_Punch list. Strategic roadmap lives in `DIRECTION.md`; this file tracks near-term tactical items._

## Architecture decisions (locked this session, 2026-05-13)

- [x] Three-layer architecture: facets / schemes / IO with paradigm-per-layer
- [x] Facets are discrete, generic, no inter-facet relations
- [x] Schemes are minimal: composed facets + IO spines + dispatch decls
- [x] IO is the single boundary substrate; namespace `io.{sim,http,ws,sms,...}.*`
- [x] Sim is deterministic + replayable (seeded PRNG)
- [x] IO tracked as effect; full algebraic effects with handlers
- [x] Multi-handler dispatch: parallel / sequence / async; 0-handler = compile error
- [x] Syntax: Go-style braces, `///` comments, `~`/`~>`/`|>`/`->`, no ceremony
- [x] Comment form: only `///` (single + multi-line via paired `///`)

## SPEC.md sections to draft

- [ ] §1 — Lexical structure (formalize what the lexer accepts)
- [ ] §2 — Type system (records, sums, generics, refinement, Option/Result, kinship at scheme level, effect types)
- [x] §3 — Facet grammar (rewritten in lockstep with parser this session)
- [ ] §4 — Scheme grammar (sketched; needs hardening as parser lands)
- [x] §5 — Wire semantics (folded into §4/§6 — no separate wire layer)
- [ ] §6 — IO grammar (sketched; needs hardening)
- [ ] §7 — Error & effect model
- [ ] §8 — Stdlib facet taxonomy
- [ ] §9 — Worked examples (the seed corpus; first one in `examples/agent.urchin`)

## Resolved §3.9 questions (this session)

- [x] §3.9.A — Multi-kinship _(dissolved — facets do not relate to each other)_
- [x] §3.9.B — `~>` distinct from `=` _(both reasons: greppability + journal hook)_
- [x] §3.9.D — Comment policy _(only `///`)_
- [x] §3.9.E — Implicit vs explicit sections _(implicit, identified by syntactic shape)_
- [ ] §3.9.C — Test block syntax _(open; defer until handler/IO semantics stable)_

## Implementation — current state

Parser ships in 3 commits this session:

- [x] `65ed4c9` — first parse loop: facet + state + dotted-path types
- [x] `8ef8b63` — function types, interface methods, handler headers
- [x] `1a0cae5` — handler bodies: expressions, assigns, reply, precedence

46 parser tests green. `urchin parse examples/episodic_memory.ur` works end-to-end.

## Implementation — next slices

- [ ] **Comparisons + conditionals + broadcast** — `>` `<` `==`, `if/else`, `broadcast Msg(args?)`. Unblocks reactive cognition (`if level > 7 { broadcast Wants }`).
- [ ] **List types `[T]` + literal `[a, b]`** — unblocks `~ episodes: [Episode]` for real instead of the `int` placeholder.
- [ ] **Pipe chains end-to-end** — named arguments (`filter(by: c)`) so the lightsaber `traces |> filter(by: c) |> reply` works.
- [ ] **Scheme declarations** — start §4 grammar in the parser.
- [ ] **IO spine declarations** — `name: io.<path>` syntax for schemes.

## Implementation — deeper choices ahead

- [ ] Decide effect-set delimiter (`{}` vs `[]` vs `<>`)
- [ ] Decide newline-significance policy (multi-line expression chains)
- [ ] LSP server scaffold — `tower-lsp` in `crates/lsp/`, JetBrains-forward (semantic tokens, inlay hints, code lens, document symbols)
- [ ] IntelliJ plugin scaffold — `editors/intellij/` Kotlin plugin that bundles `urchin-lsp`
- [ ] VS Code extension scaffold — `editors/vscode/`
- [ ] Incremental computation — `salsa` for the typechecker
- [ ] Diagnostics — wire `ariadne` or `miette` for proper error rendering

## Project hygiene

- [ ] Decide: fresh `README.md`, or `DIRECTION.md` as repo entry point
- [ ] Decide: revise `LICENSE` for a language/runtime project
- [ ] Decide: archive older project memories from the pre-language Urchin framing
- [x] Pick first `urchin` CLI subcommand — `urchin parse <file>` is live

---

_When an item completes, mark `[x]`. For SPEC sections, also annotate the SPEC.md TOC status._
