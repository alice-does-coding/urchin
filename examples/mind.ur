/// mind.ur — the urchin's first actor. composes three roles into a
/// minimal cognitive agent: an EpisodicMemory that records events,
/// a Hunger drive that tracks need, and a Voice that signals to siblings.
///
/// the actor is intentionally tiny — there is no actor-level behavior
/// code. all algorithm is emergent from the role mix. `on Tick sequence(...)`
/// is the only ceremony, declaring how multiple Tick handlers fire when
/// composition creates ambiguity.

role Hunger {
  ~ level: int

  on Tick {
    level = level ~> level + 1
    if level > 7 {
      broadcast Wants
    }
  }
}

role EpisodicMemory {
  record: Event -> Unit
  recall: Cue -> [Episode] / {io.sim.comms}

  ~ episodes: [Episode]

  on Event e {
    episodes = episodes ~> episodes + [e]
  }

  on Cue c {
    matches = episodes
      |> filter(by: c)
      |> rank(by: c.weight)
    reply matches
  }
}

role Voice {
  on Tick {}
}

actor Mind {
  EpisodicMemory
  Hunger
  Voice

  on Tick sequence(Hunger -> Voice)

  clock:    io.sim.clock
  siblings: io.sim.comms.peer
}
