// Abstract redaction: boolean “forbidden marker present” in staged text;
// redact clears the marker from the outbound projection.
module redaction_completeness {
  var staged_forbidden: bool
  var redacted_forbidden: bool
  var saw_forbidden_input: bool

  action init: bool = all {
    staged_forbidden' = false,
    redacted_forbidden' = false,
    saw_forbidden_input' = false
  }

  action stage: bool = {
    nondet bad = oneOf(Set(false, true))
    all {
      staged_forbidden' = bad,
      redacted_forbidden' = redacted_forbidden,
      saw_forbidden_input' = saw_forbidden_input
    }
  }

  action redact: bool = all {
    redacted_forbidden' = if (staged_forbidden) false else staged_forbidden,
    saw_forbidden_input' = if (staged_forbidden) true else saw_forbidden_input,
    staged_forbidden' = false
  }

  action step: bool = any {
    stage,
    redact
  }

  val redacted_is_clean: bool = not(redacted_forbidden)

  run redactClearsForbidden = {
    init.then(stage).then(redact).expect(not(redacted_forbidden))
  }
}
