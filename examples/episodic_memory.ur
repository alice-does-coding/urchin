/// episodic_memory.ur — the urchin's first dogfood role from the
/// research domain. an EpisodicMemory records events as they happen
/// and replies to cues with what it remembers.
///
/// some shapes are stand-ins until later grammar slices land:
///   - `count` is an int because list types don't exist yet —
///     the real role would carry `~ episodes: [Episode]`.
///   - `Unit` is a placeholder for the empty-return type.
///   - the `Cue -> int` reply is a stand-in for `Cue -> [Episode]`.

role EpisodicMemory {
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
