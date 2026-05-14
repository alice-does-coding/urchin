/// mind.ur — seed corpus for urchin. two actors that demonstrate
/// the language's shape end-to-end:
///
///   rubberDuck — a persona-level agent. listens to utterances from
///                the world, runs them through an LLM-backed prompter,
///                emits a clarifying question. has-a mind.
///   mind       — the cognitive substrate inside rubberDuck. composes
///                hunger + episodicMemory + voice. declared with
///                `@ rubberDuck` to position it as rubberDuck's mind
///                slot in the actor topology.
///
/// the topology is read from each actor's `@ parent` clause. siblings
/// (other children of the same parent) and children (other actors with
/// `@ thisActor`) are inferred globally from the union of declarations.
/// the tree is structural identity; capabilities flow through `io.*`
/// spines, not through the parent chain.
///
/// every identifier in urchin is camelCase. the reader infers what
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
  recall: cue -> [episode]

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
      calm     -> mood = mood ~> calm
      anxious  -> mood = mood ~> anxious
      excited  -> mood = mood ~> calm
      _        -> {}
    }
  }

  on wants need {
    mood = mood ~> anxious
  }
}

role listener {
  ~ heard: [utterance]

  on utterance u {
    heard = heard ~> heard + [u]
  }
}

role prompter {
  ask: utterance -> question / {io.anthropic.haiku}

  ~ recentQuestion: question

  on utterance u {
    q = ask(u)
    recentQuestion = recentQuestion ~> q
    broadcast question(q)
  }
}

role mouth {
  on question q {}
}

actor rubberDuck {
  llm:      io.anthropic.haiku
  siblings: io.sim.comms.peer

  listener()
  prompter(llm)
  mouth()

  on siblings.utterance sequence(listener -> prompter)
}

actor mind @ rubberDuck {
  clock: io.sim.clock

  episodicMemory(clock)
  voice(clock)(episodicMemory -> recall)
  hunger(clock)

  on clock.tick sequence(hunger -> voice)
}
