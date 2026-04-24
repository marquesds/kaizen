// Derived `skills_used` / `rules_used` rows: every stored slug obeys the same grammar
// as `src/store/event_index.rs` (`is_valid_slug` / `SLUG_RE`).
// Uses `List[str]` as a character list because Quint has no per-codepoint `str` API.
// Skip codegraph "CONTAINS" and CLI `--target-pct` (not this state machine).

module event_index {
  // Rust: r#"^[A-Za-z0-9][A-Za-z0-9_\-]{0,63}$"#
  pure val SLUG_FIRST: Set[str] = Set("a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9")
  pure val SLUG_REST: Set[str] = Set("a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "_", "-")

  pure def is_valid_slug_list(chars: List[str]): bool = all {
    length(chars) > 0,
    length(chars) <= 64,
    head(chars).in(SLUG_FIRST),
    indices(chars).exclude(Set(0)).forall(i => nth(chars, i).in(SLUG_REST)),
  }

  // Finitely many positive samples (invalid shapes like _leading, ".", or "." in body
  // are rejected by `is_valid_slug_list` and are not included here so ingest always enables).
  pure val SLUG_LIST_SAMPLES: Set[List[str]] = Set(
    List("a"),
    List("0"),
    List("Z"),
    List("A", "b", "c", "_", "9", "-", "Z")
  )

  var skills_rows: Set[(str, List[str])]
  var rules_rows: Set[(str, List[str])]

  action init: bool = all {
    skills_rows' = Set(),
    rules_rows' = Set(),
  }

  // Implementation path: only insert when the slug list is well-formed.
  action ingest_skill: bool = {
    nondet sid = oneOf(Set("sess_a", "sess_b"))
    nondet chs = oneOf(SLUG_LIST_SAMPLES)
    all {
      is_valid_slug_list(chs),
      skills_rows' = skills_rows.union(Set((sid, chs))),
      rules_rows' = rules_rows
    }
  }

  action ingest_rule: bool = {
    nondet sid = oneOf(Set("sess_a", "sess_b"))
    nondet chs = oneOf(SLUG_LIST_SAMPLES)
    all {
      is_valid_slug_list(chs),
      rules_rows' = rules_rows.union(Set((sid, chs))),
      skills_rows' = skills_rows
    }
  }

  action step: bool = any {
    ingest_skill,
    ingest_rule
  }

  // Post-state: every row's slug (as char list) matches `event_index::is_valid_slug` / `SLUG_RE`.
  val all_slugs_well_formed: bool = all {
    skills_rows.forall(r => is_valid_slug_list(r._2)),
    rules_rows.forall(r => is_valid_slug_list(r._2))
  }

  run testIndexedSlugsWellFormed = {
    init
      .then(ingest_skill)
      .then(ingest_rule)
      .expect(all_slugs_well_formed)
  }
}
