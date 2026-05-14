/// hunger.ur — the urchin's smallest dogfood role.
/// targets the current grammar slice: interface methods, state fields,
/// and handler headers. handler bodies (expressions, pipes, mutations)
/// land in the next slice.

role Hunger {
  is_satisfied: Tick -> Bool

  ~ level: int

  on Tick {}
}
