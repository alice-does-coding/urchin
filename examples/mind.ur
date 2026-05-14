/// mind.ur — the urchin's first actor. composes three roles into a
/// minimal cognitive agent: an episodicMemory that records events,
/// a hunger drive that tracks need, and a voice that signals to siblings.
///
/// the actor body has three sections in canonical order:
///   1. IO spines (the substrate the actor talks to)
///   2. role instances with their IO + role-to-role wiring
///   3. orchestration (dispatch declarations on spine.event)
///
/// every identifier in urchin is camelCase — including role names,
/// message types, and constructor patterns. the reader infers what
/// kind of thing a name refers to from syntactic position.

role hunger {
  ~ level: float

  on tick {
    level = level ~> level + 0.01
    if level > 0.7 {
      broadcast wants("food")
    }
  }
}

role episodicMemory {
  record: event -> unit
  recall: cue -> [episode] / {io.sim.comms}

  ~ episodes: [episode]

  on event e {
    episodes = episodes ~> episodes + [e]
  }

  on cue c {
    matches = episodes
      |> filter(by: c)
      |> rank(by: c.weight)
    reply matches
  }
}

role voice {
  ~ mood: mood

  on tick {
    match mood {
      calm     -> broadcast hum
      anxious  -> broadcast whisper
      excited  -> broadcast shout
      _        -> {}
    }
  }
}

actor mind {
  clock:    io.sim.clock
  siblings: io.sim.comms.peer

  episodicMemory(clock, siblings)
  voice(clock)(episodicMemory -> recall)
  hunger(clock)

  on clock.tick sequence(hunger -> voice)
}
