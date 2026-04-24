// Redaction: forbidden content (prompts, completions, raw paths, emails, secrets) is never
// emitted; allowlisted identity labels (e.g. `team`, `workspace_label`, …) only appear
// in cleartext when the policy explicitly allowlists that label; otherwise they are omitted or hashed.
module redaction_completeness {
  var staged_forbidden: bool
  var redacted_forbidden: bool
  var saw_forbidden_input: bool
  var allowlist_team: bool
  var staged_cleartext_team: bool
  var out_cleartext_team: bool

  action init: bool = all {
    staged_forbidden' = false,
    redacted_forbidden' = false,
    saw_forbidden_input' = false,
    allowlist_team' = false,
    staged_cleartext_team' = false,
    out_cleartext_team' = false
  }

  // Policy flip (e.g. config load) while not mid-stage.
  action set_allowlist_team: bool = {
    nondet on = oneOf(Set(false, true))
    all {
      staged_forbidden' = staged_forbidden,
      redacted_forbidden' = redacted_forbidden,
      saw_forbidden_input' = saw_forbidden_input,
      allowlist_team' = on,
      staged_cleartext_team' = staged_cleartext_team,
      out_cleartext_team' = out_cleartext_team
    }
  }

  action enable_team_allowlist: bool = all {
    allowlist_team' = true,
    staged_forbidden' = staged_forbidden,
    redacted_forbidden' = redacted_forbidden,
    saw_forbidden_input' = saw_forbidden_input,
    staged_cleartext_team' = staged_cleartext_team,
    out_cleartext_team' = out_cleartext_team
  }

  action stage: bool = {
    nondet bad = oneOf(Set(false, true))
    all {
      staged_forbidden' = bad,
      redacted_forbidden' = redacted_forbidden,
      saw_forbidden_input' = saw_forbidden_input,
      allowlist_team' = allowlist_team,
      staged_cleartext_team' = staged_cleartext_team,
      out_cleartext_team' = out_cleartext_team
    }
  }

  // Stage only a team label (no forbidden bits) for allowlist tests.
  action stage_team_label: bool = all {
    staged_forbidden' = false,
    redacted_forbidden' = redacted_forbidden,
    saw_forbidden_input' = saw_forbidden_input,
    allowlist_team' = allowlist_team,
    staged_cleartext_team' = true,
    out_cleartext_team' = out_cleartext_team
  }

  action redact: bool = all {
    redacted_forbidden' = if (staged_forbidden) false else staged_forbidden,
    saw_forbidden_input' = if (staged_forbidden) true else saw_forbidden_input,
    staged_forbidden' = false,
    allowlist_team' = allowlist_team,
    out_cleartext_team' = (not(staged_forbidden) and allowlist_team and staged_cleartext_team),
    staged_cleartext_team' = false
  }

  action step: bool = any {
    set_allowlist_team,
    enable_team_allowlist,
    stage,
    stage_team_label,
    redact
  }

  val redacted_is_clean: bool = not(redacted_forbidden)

  val team_cleartext_requires_allowlist: bool = out_cleartext_team implies allowlist_team

  run redactClearsForbidden = {
    init.then(stage).then(redact).expect(not(redacted_forbidden))
  }

  // Team label is omitted from cleartext output when the allowlist is off.
  run team_omitted_without_allowlist = {
    init
      .then(stage_team_label)
      .then(redact)
      .expect(not(out_cleartext_team) and not(redacted_forbidden))
  }

  // With allowlist on, a staged team label can appear in cleartext.
  run team_cleartext_when_allowed = {
    init
      .then(enable_team_allowlist)
      .then(stage_team_label)
      .then(redact)
      .expect(out_cleartext_team and allowlist_team and not(redacted_forbidden))
  }
}

