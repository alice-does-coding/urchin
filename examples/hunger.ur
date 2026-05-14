/// hunger.ur — the urchin's reactive shape. each tick the level rises;
/// once it crosses the threshold, a Wants signal goes out for any
/// composed sibling role to pick up. shows conditionals and broadcast
/// against the existing state-shift / addition / comparison machinery.

role Hunger {
  ~ level: int

  on Tick {
    level = level ~> level + 1
    if level > 7 {
      broadcast Wants
    }
  }
}
