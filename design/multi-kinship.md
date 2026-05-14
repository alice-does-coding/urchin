# Design investigation: multi-kinship — RETIRED

_Started 2026-05-13. Retired same day._

## Resolution

**The question dissolved.** Roles in Urchin do not relate to each other at all — there is no kinship, inheritance, or abstract/concrete distinction at the role level. Cross-cutting capabilities are composed at the actor level, not inherited at the role level.

See `SPEC.md` §0.1 (Architecture) and §3 (Role grammar) for the current model. The original investigation surfaced the right questions; the answer turned out to be that roles being too inexpressive about each other was the right move (Squeak Traits + actor-model stance), and the abstraction-and-reuse work moves to the actor and IO layers.

This file remains as a record of the investigation that led to the dissolution. The contents below are the original investigation, preserved for context — none of it is current design.

---

_(Original investigation contents removed at retirement. See git history at commit `8ef8b63` and earlier for the full text.)_
