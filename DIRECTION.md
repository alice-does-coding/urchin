# Urchin — Direction

_Captured 2026-05-13 from a late-night co-design session. This document supersedes the earlier "port Garden Arcade to Elixir/Phoenix" framing. The existing Elixir scaffolding (`mix.exs`, `lib/`, etc.) is from that earlier direction and will be re-evaluated; the new direction is to **design and build our own language and runtime**, with Go as the host language for the runtime._

---

## What urchin is

**Urchin is a typed actor language designed for AI-collaborative authorship from day one, in which roles compose into actors, actors speak comms, and wires can cross arbitrary boundaries — machine, network, browser-vs-server.**

The flagship use case is **building cognitive substrates for agents** (Garden Arcade's successors). The general use case is **any system with state + IO + concurrency + multiple cooperating units**, which it turns out is most of programming — including web apps, distributed systems, and games. The cognitive-substrate framing gives the language its biology and identity; the generality is what makes it a real stack rather than a toy.

---

## The core insight: AI-collab-first

Most languages are designed first, then ML models eventually become fluent by accident from accumulated public code over decades. By the time the model is fluent, the language is already old. **Urchin inverts that:** AI collaboration is a primary design constraint, alongside the usual concerns (correctness, performance, ergonomics). The AI is a co-designer of the language AND its first fluent user, by construction.

The standard "AI struggles with low-corpus languages" failure mode is: paradigm-distant language → guess at idioms → drift from canonical shape → compiler can't catch the drift in time. Four compounding problems. Urchin inverts all four:

1. **Paradigm distance becomes a feature.** Roles/actors/comms isn't distant from how AI reasons about agents — it's close to how the cognitive concepts already live in models. The syntax is the thinnest part; conceptual fluency is already there.
2. **Idioms aren't a guess.** Abstract roles define expected interfaces. Concrete roles implement them. Actors wire concrete roles together. Exactly one shape per thing. Canonical IS the language, not an unwritten community norm.
3. **The seeded corpus replaces the missing organic corpus.** ~30 stdlib roles + ~10 example actors + ~5 comms ≈ 40–80kb of source. The AI holds the entire stdlib in context. Pattern-matching against the actual canonical corpus that's right there in the prompt, not against vague training-data memories.
4. **Strong static checking catches drift instantly.** Strong static + small units + standardized roles = drift caught at the syntax level.

The recursive corollary: **AI is fluent on day one because the language was designed for it.** This has not been done before.

---

## The three units

**`role`** — the basic compositional unit. One cognitive faculty (associative memory, working memory, hunger drive, attention, voice, embodiment, etc.). Small, self-contained, hard interface. Roughly equivalent to a brain region.

**`actor`** — an executable. An assembly of roles wired together. Each actor is one process / one agent. The wiring IS the topology — which roles signal to which. Like a circuit diagram, or like the projections between brain regions.

**`comms`** — the inter-actor layer. What actors say to each other. A separate abstraction from role-to-role signaling because the latencies, semantics, and trust models are different. (Synapses inside a brain ≠ social signals between brains.)

### Example

```
role AssociativeMemory : Memory.Associative
  recall:  Query -> [Trace]
  store:   Trace -> ()
  ~ traces: [Trace]

  on Query q:
    matches = traces |> filter(by: q.cue) |> rank(by: q.weight)
    reply matches

role Hunger : Drive
  ~ level: 0..1

  on Tick:
    level = level ~> min(1.0, level + 0.01)
    if level > 0.7: broadcast Wants(Food)

actor PizzaMan
  roles:
    mem:        AssociativeMemory
    hunger:     Hunger
    voice:      Voice.Text
    embodiment: Embodiment.Image(handle: "pizza-man")
    attention:  Attention.Salience

  wire:
    hunger.Wants    -> attention.in
    attention.focus -> mem.Query
    mem.Trace       -> voice.speak
    voice.uttered   -> mem.store

  speaks: Garden
```

Read top to bottom: what brain regions does this agent have, and how are they connected. That's the spec.

---

## The urchin metaphor (biology → semantics)

The name is not decorative. Sea urchin biology is load-bearing for the language design:

- **Radial symmetry, spines radiating from a center** → roles radiate from the actor's bus. No role is privileged. No "head" role.
- **No brain, just a nerve ring around the mouth** → there's no central executive. Thin coordination (the runtime scheduler) plus local action. **Actor-as-coordination, not actor-as-controller.** The actor is emergent.
- **Water vascular system, hydraulic, pressure-driven** → pub/sub bus is the native messaging medium. Signals flow through a shared internal medium; roles respond to pressure changes, not direct calls. No role "calls" another.
- **Photoreceptors distributed across the entire body** → sensing is parallel and ambient. Any role can declare itself as a receptor for external input. Distributed perception is the default.
- **Each spine moves toward a threat autonomously** → roles can act on local information without round-tripping through the actor. Reflexes bypass deliberation.
- **Aristotle's lantern (the chewing apparatus)** → the *one* complex centralized organ. Mapped to the comms layer — the inter-actor I/O boundary. Ceremony is allowed here and only here.
- **Fronts and aggregations** → urchin fronts grazing kelp together = the cast. Agent colony with no central choreography, group behavior emergent from local comms + spatial proximity. **Spatial / topological neighborhoods are first-class in the comms layer.**
- **Regenerative — they regrow spines** → roles are hot-swappable in a running actor. Native syntax for role swap with state migration, not an advanced topic.
- **Test (shell) built from local secretions** → roles can extend the actor's structure from within. New role declares its own state and wire endpoints; the actor doesn't have to be updated to know about its parts.
- **~500 million years old** → the language should feel slow, persistent, ancient. Stone-grade code. Not "rewrite every 18 months."
- **Defensive sovereignty without aggression** → each actor is inviolable, the comms is a politeness, the test is the boundary. No API-gateway energy. No service-mesh energy. Passive, polite, sovereign.
- **Economy of form (few part types, repeated)** → the stdlib should be small. ~20 role types covering all of cognition, not 200. Phonemes vs. words. Write 20 roles, build 10,000 actors.

---

## Language design principles

The four variables that determine "how well does AI collaborate on stack X":
1. **Training corpus size**
2. **Idiom uniformity**
3. **Paradigm distance from median programming**
4. **State required across tokens** (lifetimes, supervision topology, monadic stacks)

Urchin design targets each:

- **Functions/roles are closed units.** All inputs explicit. All effects declared. Reading a role is sufficient to know what it does. Zero "you have to know the supervision tree topology to know if this is right."
- **Errors are values, not exceptions.** If a function can fail, the return type says so.
- **One canonical form per task.** No five ways to write a loop. The language enforces it.
- **Generative skeletons.** Every construct has fill-in-the-blanks shape. `role Foo : SomeAbstractRole { ... }`, `actor Bar { roles: ..., wire: ..., speaks: ... }`, `comms Baz { ... }`. Uniform skeleton across all units means the AI completes inside a known frame, never staring at a blank file.
- **Names carry type info syntactically.** Required predicate prefixes (`is_`, `has_`, `can_`), required event suffixes (`*_at` for times, `*_id` for refs). The parser rejects misnamed functions. Naming becomes free training signal.
- **Effects in types.** A function's signature tells you what it can do. No reading the body to learn whether it touches the DB.
- **LSP-first, not editor-second.** The compiler exposes type info, dataflow, dep graph, test status as a first-class API the AI queries before answering.
- **Kinship in the types.** `Sibling[A, B]` is a real type relation the compiler tracks. The topology is IN the source.
- **No nullable types.** Option/Maybe always.
- **Structural over nominal types.** `{ name: String, age: Int }` beats `User implements IUser`.
- **Tests adjacent to functions, syntactically.** Every role has a `test` block. The spec lives next to the code.
- **Typed comments.** `@invariant`, `@assumes`, `@because` for machine-readable rationale. No free-form `//` for important things.

---

## Syntax direction: terse + regular

Two kinds of terse:
- **Regular terse** (Haskell, Elm, OCaml): short, but every form is consistent. AI handles this well.
- **Chaotic terse** (APL, Perl golf): symbol soup. AI is bad at this.

We want regular terse. Density measured in **tokens**, not characters. Compression comes from **removing whole categories of ceremony**, not from one-letter keywords. Type inference everywhere the compiler can do it. Implicit returns from the last expression. No `;`, no `,` between fields when newlines do the job. Punctuation operators for the things that recur dozens of times per file:

- `!` for send (Erlang convention)
- `?` for receive
- `:` for parent-of / kinship
- `~>` for state shift
- `|>` for pipe (universal)
- `<-` for bind / await

Short structural keywords (`role`, `actor`, `comms`, `on`, `use`, `let`, `match`, `wire`, `speaks`), not verbose ones (`function`, `declare`, `implements`).

**Critical pairing:** terseness must come with **strong immediate static checking**. Terse + dynamic = APL pain. Terse + strict-static = Elm joy.

**The payoff:** Garden Arcade's `BetaGate.jsx` is ~600 lines. A terse-language equivalent ≈ 150 lines. **4x density improvement in what the AI holds in context at once.** For a long codebase, this is the difference between "AI sees three files" and "AI sees the whole subsystem."

---

## The frontend+backend unification (the deep thing)

Every system that has state + IO + concurrency + multiple cooperating units has the same shape underneath. Brains, web apps, distributed systems, games, OSes. The actor model is general because the constraints are universal.

A web app is two actors:

**Backend actor:**
- Roles: auth, db-access, business-logic-per-domain, cache, rate-limit, audit-log, queue-worker
- Wires: request → auth → business → db → cache
- Comms: HTTP/WS

**Frontend actor (per browser tab):**
- Roles: routing, network, ui-state, input-handling, persistence(localStorage), animation, view-rendering
- Wires: input → ui-state → view; network → ui-state
- Comms: speaks the same HTTP/WS to backend actors

Same shape. The only differences are runtime environment, environment-specific roles, and a network in between.

**The killer move:** in urchin, **the network is just a wire that crosses a machine boundary.** When a frontend role declares `network.fetch -> ui_state.update`, that wire serializes the message, ships it across the wire, gets handled by a backend role, and the reply comes back. **There is no separate API.** The wire IS the API. The comms contract is generated from the wire declarations on both sides, automatically.

```
actor GardenArcadeServer
  roles:
    feed:   feed.Server
    chat:   chat.Server
    cache:  cache.LRU(1000)
  wire:
    feed.request -> cache.query
    cache.miss   -> feed.fetch
    chat.tick    -> inference.invoke
  speaks: Arcade

actor GardenArcadeBrowser
  roles:
    feed_ui:  feed.UI
    chat_ui:  chat.UI
    net:      net.WebSocket(to: GardenArcadeServer)
  wire:
    feed_ui.scroll -> net.send(Arcade.FeedRequest)
    net.recv       -> feed_ui.update
  speaks: Arcade
```

`speaks: Arcade` on both sides. The compiler reads both actors, derives the comms contract from messages each side sends and expects, generates wire serialization, and you're done. **No swagger, no protobuf, no GraphQL schema, no tRPC, no OpenAPI.** The language contains the contract because the wire syntax already declares it.

### What falls out of unification

- **Hot reload across the network boundary.** Swap a backend role; the frontend's wire endpoint adapts because the comms is regenerated.
- **Role testing is identical on both sides.** Test-harness doesn't care whether the role lives in a browser or on a server.
- **"Server vs. client component" becomes "where does this role live."** One config decision per role, not an architectural pivot.
- **The developer never thinks about REST/RPC/serialization.** Compiler concern.

Phoenix LiveView half-solves this. Meteor tried. Isomorphic JS tried. Remix/Next dance around it. They all started from existing language primitives that aren't the right shape and bolted actors / state / messaging on top. **Urchin starts from the right primitive**, so the unification falls out instead of being engineered.

---

## Runtime decision: write our own, not BEAM

**BEAM gets us 70% of what we need for free** — lightweight processes, supervision, hot reload, message passing, distribution, 30 years of production hardening. We could ship in 2–3 months by writing a parser + transpiler to BEAM bytecode or Core Erlang.

**But BEAM constrains:**
- PIDs are opaque flat tokens. Kinship types want *typed* relationships the runtime understands, not a process-registry lookup we layer on top.
- BEAM's process model assumes telecom-shape workloads: high throughput, short messages, latency-bounded. Urchins are slow grazers. We'd be paying for primitives we don't need.
- Supervision in BEAM is OTP supervisors with restart strategies. We want *biological* regeneration semantics — roles have their own regrowth logic; the actor is emergent.
- Mailbox semantics (one mailbox per process, ordered delivery) don't fit pub/sub-on-a-bus. Forcing pub/sub onto BEAM mailboxes is ugly, and ugly leaks upward into the language.
- BEAM's hot reload is module-level. We want role-level swap with state migration.

**Writing our own gets us:**
- Kinship/role types as first-class runtime concepts, not type-system bandaids.
- Pub/sub bus as the native message medium.
- Spatial topology (which actors are "near" which) as a runtime primitive — huge for urchin-front group behavior.
- Role lifecycle shaped to biology (secrete, integrate, regenerate, hot-swap), not OTP.
- Wild affordances BEAM can't do: every message journaled by default, every state change reversible, deterministic replay for debugging.

**The runtime is part of the artwork.** Garden Arcade is JS/Go. The next game is made in YOUR language on YOUR substrate. The trilogy completes in its own substrate.

**Honest scope:** a BEAM-grade runtime is decades of engineering. **We're not building that.** We're building a runtime for *thousands of actors, not millions of processes.* Small-but-correct, not big-but-fast. ~100x narrower in scope than BEAM. Feasible for solo-plus-AI over ~a year, especially with AI-collab-first design making each component small and legible.

**Trade we're accepting:** urchin programs will be slower than equivalent BEAM programs by 10–50x raw throughput. For slow-grazing actors, invisible. For telecom switching, disqualifying — but we're not building telecom.

---

## Host language: Rust

**The compiler and runtime are written in Rust.** One language for everything: parser, typechecker, LSP server, runtime, stdlib role implementations, CLI, test harness. Single Cargo workspace, no language boundary.

Reasoning:

- **Compiler is the bigger half.** Parser + typechecker + LSP (~5-10k lines) is larger than the runtime (~3-5k lines). Compiler work is ADT/pattern-match heavy — Rust's enums, exhaustive match, and ownership-tracked AST traversals make it meaningfully cleaner than alternatives that rely on type-switching or unsafe casts. We want the host to shine where most of the work lives.
- **Runtime work is doable at this scale.** For "thousands of actors, not millions," Rust's async + Arc/Mutex/channels are workable for the bus, registry, and spatial topology. Less ergonomic than goroutines, but bounded by the runtime's smallness.
- **LLM corpus is right-shaped and growing.** Rust's compiler/language-tools subset (rustc, chumsky, salsa, miette, tower-lsp, ungrammar) is exactly the kind of code being written here. Frontier models get more fluent here every quarter. This aligns with the LLM-native design constraint above.
- **Single binary, no GC tax, no runtime dependency.** Distribution is "ship this file."
- **Artwork fit.** Rust's culture (correctness, ownership-as-sovereignty, no GC) maps onto the urchin metaphor — slow, ancient, sovereign, stone-grade — more naturally than the alternatives. The substrate is part of the work.

**Alternatives considered and rejected:**

- **Go** — pragmatic, fast inner loop, goroutines-as-actors mapping is direct, and the existing project author knows it. Rejected because the compiler half pays a real verbosity tax in Go (no sum types, no exhaustive match, AST traversal via interface assertions), and the compiler is the larger half of the codebase. Familiarity was set aside intentionally as a soft factor.
- **OCaml** — beautiful for parsers and typecheckers, but LLM corpus is small and not growing. Disqualified by the LLM-native design constraint.
- **Zig** — same corpus problem as OCaml, plus a less mature stdlib.
- **TS/Bun** — surprisingly viable for the compiler half, but the runtime story (single-threaded event loop, no real lightweight processes) is wrong shape for an actor runtime.
- **Rust compiler + Go runtime split** — appealing on paper but rejected. Doubles build systems / CI / test frameworks; introduces a serialization boundary at the compiler/runtime interface (the tightest coupling in the system); splits the AI fluency profile; breaks the "urchin as coherent organism" rhetoric. The pattern works at scale; not at solo-plus-AI scale.

**Honest cost accepted:** slower inner loop than Go (rustc incremental compiles in the 5–15s range for ~10k LOC). Mitigated by cargo's incremental compilation + watch tooling and by splitting the compiler frontend into its own crate so runtime/LSP changes don't trigger full rebuilds.

**Rust for everything:** parser, typechecker, LSP, runtime, stdlib role implementations, CLI, test harness. One language. One mental model. One AI fluency profile. One binary.

---

## The seeded canonical corpus

A 10,000-example seeded corpus solves the cold-start problem for AI fluency. We curate it *as the language is designed*, alongside the spec. Every design decision tested against "is this AI-completable?" before it locks in. Every ambiguity resolved toward "what makes the AI's job easier."

The corpus covers:
- All ~20 stdlib role types, with 2–3 example role implementations each
- ~10 example actors across different domains (cognitive agent, web backend, web frontend, distributed worker, game state machine, IoT mesh)
- ~5 example comms (Garden-style agent comms, HTTP-like RPC, pub/sub spatial)
- ~50 documented design patterns ("how do I add episodic memory to an actor," "how do I split a role across machines")

**Property:** the full corpus fits in 80–150kb of source. **The AI can hold the entire canonical world in its context window.** When you ask it to write a new role, it's not pattern-matching against vague training-data memories — it's pattern-matching against the actual stdlib in the prompt.

This has never been done. Languages have been designed for humans, compilers, type systems, performance, academic elegance. Nobody has designed a language with **"the AI assistant must be fluent on day one"** as a primary constraint.

---

## Roadmap

This is a multi-year substrate, sketched alongside Garden Arcade and the post-GA arc. Not "build after GA ships" — sketched continuously, becomes the substrate for what comes next.

### Phase 0 — Spec + seed corpus (months 1–3)
1. Write `SPEC.md` together over a few sessions: grammar in BNF-ish, semantic notes, role taxonomy, wire syntax. Maybe 30–50 pages. Every decision documented with rationale.
2. Hand-write 5 example actors and 15 example roles in the spec language, in markdown. Nothing runs yet. **This IS the seed corpus.**
3. Run "can the AI complete this from a partial sketch" tests against the spec + corpus. Iterate spec until completions feel right.

### Phase 1 — Parser + typechecker (months 3–6)
4. Parser in Rust (`urchin/parser/`). AST only, no semantics.
5. Typechecker (`urchin/types/`). Roles, kinship, wire-type-fitting, effect inference, naming-convention enforcement.
6. LSP server (`crates/lsp/`, Rust + `tower-lsp`) — primary editor target is JetBrains (IntelliJ plugin in `editors/intellij/` bundles the binary and registers `.ur` files via JetBrains' LSP support). VS Code as secondary minimal target (`editors/vscode/`). Exposes type/dataflow/test info as a queryable API. Rich capabilities from the start: semantic tokens, inlay hints, code lens, document symbols, signature help.

### Phase 2 — Runtime (months 6–9)
7. Runtime in Rust (`urchin/runtime/`). Async task per actor (tokio), broadcast-channel bus per actor, role dispatch loop, hot-swap primitive, spatial topology registry.
8. CLI: `urchin run actor.ur` parses, checks, and runs.
9. Stdlib role implementations for the basic abstract roles (~20 roles covering Memory.*, Drive.*, Attention.*, Voice.*, Embodiment.*, Sensory.*).

### Phase 3 — First real port (months 9–12)
10. Port one Garden Arcade agent to urchin. Probably the kettle or pizza man — small, observable.
11. Stand up a local urchin runtime alongside the GA stack. The agent runs in urchin; GA's HTTP layer talks to it via a temporary comms bridge.
12. Validate the developer experience end-to-end. Iterate.

### Phase 4 — Frontend wires (months 12+)
13. Browser runtime (compile a subset of urchin to JS, or run a tiny wasm interpreter).
14. Cross-machine wires: declare a wire in the language, compiler generates serialization + transport.
15. Build a small web app entirely in urchin (frontend + backend in one language) as the first proof of the unification.

### Phase 5 — Native game (years 2+)
16. Build urchin-game (whatever you call the next thing) natively in urchin. **The language proves itself by being the substrate of the artwork.**

---

## What this is for

This is not a side project. This is **the substrate for every agent-shaped thing Alice builds for the next decade.**

- Garden Arcade is the first iteration in JS/Go because that's what existed.
- Urchin-lang is the same idea in a substrate that thinks in the right shapes.
- Game II, Game III, whatever comes after — they all want this substrate.
- The portfolio story: "I designed and built a typed actor language with frontend+backend wire unification, and shipped two artworks on top of it" is unfakeable. Anthropic-shaped, indie-game-engine-shaped, whoever-shaped.

And the recursive-design aspect — AI as co-designer and first fluent user — is itself a thesis-grade contribution to language design. **Nobody has built a language this way.** Doing it is its own argument.

---

## Existing Elixir scaffolding

The current `lib/`, `mix.exs`, `config/`, `deps/`, `test/` are from the earlier "port GA to Phoenix/Elixir" framing. They are NOT load-bearing for the new direction. Decision deferred on whether to:
1. Keep the Elixir code around as a reference reincarnation while urchin-lang is being designed
2. Delete and start the urchin/Go scaffolding fresh
3. Move the Elixir code to a separate `legacy-port/` subdirectory

This decision is Alice's. The DIRECTION.md is the authoritative source for what the project IS now; the Elixir scaffolding is what it was on the way here.
