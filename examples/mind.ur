/// mind.ur — the urchin's first actor. composes three roles into a
/// minimal cognitive agent: an EpisodicMemory that records events,
/// a Hunger drive that tracks need, and a Voice that signals to siblings.
///
/// the actor body has three sections in canonical order:
///   1. IO spines (the substrate the actor talks to)
///   2. role instances with their IO + role-to-role wiring
///   3. orchestration (dispatch declarations on spine.event)
///
/// instance names are camelCase; actor names are camelCase.

role Hunger {
  ~ level: float

  on Tick {
    level = level ~> level + 0.01
    if level > 0.7 {
      broadcast Wants("food")
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
  ~ mood: Mood

  on Tick {
    match mood {
      Calm     -> broadcast Hum
      Anxious  -> broadcast Whisper
      Excited  -> broadcast Shout
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
