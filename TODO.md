# TODO

_Punch list. Strategic roadmap lives in `DIRECTION.md`; this file tracks near-term tactical items surfaced in design sessions._

## SPEC.md sections to draft

Section 3 (Role grammar) has a first draft. Remaining sections, in suggested order:

- [ ] §1 — Lexical structure (encoding, whitespace, tokens, keywords, operators, comments)
- [ ] §2 — Type system (primitives, records, sums, generics, function types, `[T]`, refinement types like `0..1`, Option/Result, kinship types, effect types)
- [ ] §4 — Actor grammar (`roles:`, `wire:`, `speaks:`)
- [ ] §5 — Wire semantics (`->`, fan-in/out, cross-machine wires, hot-swap)
- [ ] §6 — Comms grammar (message contracts derived from `speaks:`)
- [ ] §7 — Error & effect model (Result everywhere, no panics, effect inference)
- [ ] §8 — Stdlib role taxonomy (~20 abstract roles, interfaces only)
- [ ] §9 — Worked examples (5 actors, 15 roles — the seed corpus)

## Open spec questions to resolve

Locking these firms up §3 and unblocks dependent sections. Each has a drafted answer in `SPEC.md`.

- [ ] §3.9.A — Multi-kinship allowed (`role X : A, B`)?
- [ ] §3.9.B — Why `~>` distinct from `=`?
- [ ] §3.9.C — Test block syntax
- [ ] §3.9.D — Free-form `//` comments — banned, allowed, or lint-flagged?
- [ ] §3.9.E — Implicit vs explicit section keywords in role bodies

## Project hygiene

- [ ] Decide: fresh `README.md`, or is `DIRECTION.md` enough as repo entry point?
- [ ] Decide: does the existing `LICENSE` (carried from the GA-port era) still apply, or revise for a language/runtime project?
- [ ] Decide: keep, fold into §6 thinking, or archive the older project memories (`project-protocol-architecture`, `project-primitive-alphabet`) from the earlier Urchin framing
- [ ] Pick the first real `urchin` CLI subcommand (currently `fn main() {}`)

## Implementation

Detailed roadmap in `DIRECTION.md` (Phases 0–5). Near-term picks to flag once SPEC.md §1–4 are stable:

- [ ] Parser library choice — `chumsky` vs hand-rolled recursive descent vs `nom`
- [ ] Incremental computation — `salsa` for the typechecker
- [ ] LSP server scaffold — `tower-lsp` in `crates/lsp/`, designed JetBrains-forward (rich capabilities: semantic tokens, inlay hints, code lens, document symbols)
- [ ] IntelliJ plugin scaffold — `editors/intellij/` Kotlin plugin that bundles `urchin-lsp`, registers `.ur` files via JetBrains LSP support
- [ ] VS Code extension scaffold — `editors/vscode/`, minimal binary-spawner

---

_When an item completes, mark `[x]`. For SPEC sections, also annotate the SPEC.md TOC status (`(TBD)` → `(draft)` → `(stable)`)._
