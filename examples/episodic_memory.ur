/// episodic_memory.ur — the urchin's first dogfood role from the
/// research domain. an EpisodicMemory records events as they happen
/// and replies to cues with the episodes it remembers, ranked by the
/// cue's weight.
///
/// `Episode`, `Event`, `Cue`, `Unit` are opaque paths here because
/// record types don't exist in the grammar yet — those shapes fill in
/// when §2 lands.

role EpisodicMemory {
  record: Event -> Unit
  recall: Cue -> [Episode]

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
