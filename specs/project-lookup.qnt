// resolve_project_name: maps a short name to a registered workspace path.
// Resolution: slug-exact match first, then last-path-segment match.
// Invariants: Found ↔ exactly one match; Ambiguous ↔ multiple matches;
//             NotFound ↔ zero matches. Outcome is terminal per call.
module project_lookup {
  type Outcome = | Found | NotFound | Ambiguous

  var match_count: int
  var outcome: Outcome

  action init: bool = all {
    match_count' = 0,
    outcome' = NotFound,
  }

  val possible_counts = Set(0, 1, 2, 5)

  // Resolve with a nondeterministically chosen number of matches.
  action resolve: bool = {
    nondet n = oneOf(possible_counts)
    all {
      match_count' = n,
      outcome' = if (n == 1) Found
                 else if (n == 0) NotFound
                 else Ambiguous,
    }
  }

  action step: bool = any {
    init,
    resolve,
  }

  val found_iff_single_match: bool =
    (outcome == Found) iff (match_count == 1)

  val ambiguous_iff_multi_match: bool =
    (outcome == Ambiguous) iff (match_count > 1)

  val not_found_iff_zero: bool =
    (outcome == NotFound) iff (match_count == 0)

  run exactMatch = {
    init.then(resolve).expect(
      (match_count == 1) implies (outcome == Found)
    )
  }

  run noMatch = {
    init.then(resolve).expect(
      (match_count == 0) implies (outcome == NotFound)
    )
  }

  run ambiguousMatch = {
    init.then(resolve).expect(
      (match_count > 1) implies (outcome == Ambiguous)
    )
  }
}
