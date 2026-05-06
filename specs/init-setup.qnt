// `kaizen init`: install config + skill, patch hook files once, rerun is noop.
// Wiring target: ~/.cursor/hooks.json and ~/.claude/settings.json (user-global, not workspace).
// legacy_cursor_local / legacy_claude_local: workspace-local kaizen wiring from a prior run — detected
// by `doctor`, never removed by init. invariant: legacy implies global is also patched.
// Models six hook-oriented hosts (Cursor, Claude Code, Codex, Copilot CLI, OpenClaw, Mistral Vibe).
module init_setup {
  var config_ready: bool
  var cursor_present: bool
  var cursor_patched: bool
  var cursor_backup: bool
  var legacy_cursor_local: bool
  var claude_present: bool
  var claude_patched: bool
  var claude_backup: bool
  var legacy_claude_local: bool
  var codex_present: bool
  var codex_patched: bool
  var codex_backup: bool
  var copilot_cli_present: bool
  var copilot_cli_patched: bool
  var copilot_cli_backup: bool
  var openclaw_present: bool
  var openclaw_patched: bool
  var openclaw_backup: bool
  var vibe_present: bool
  var vibe_patched: bool
  var vibe_backup: bool
  var skill_ready: bool

  action init: bool = all {
    config_ready' = false,
    cursor_present' = false,
    cursor_patched' = false,
    cursor_backup' = false,
    legacy_cursor_local' = false,
    claude_present' = false,
    claude_patched' = false,
    claude_backup' = false,
    legacy_claude_local' = false,
    codex_present' = false,
    codex_patched' = false,
    codex_backup' = false,
    copilot_cli_present' = false,
    copilot_cli_patched' = false,
    copilot_cli_backup' = false,
    openclaw_present' = false,
    openclaw_patched' = false,
    openclaw_backup' = false,
    vibe_present' = false,
    vibe_patched' = false,
    vibe_backup' = false,
    skill_ready' = false
  }

  action seed_empty_workspace: bool = init

  action seed_existing_hook_files: bool = all {
    config_ready' = false,
    cursor_present' = true,
    cursor_patched' = false,
    cursor_backup' = false,
    legacy_cursor_local' = false,
    claude_present' = true,
    claude_patched' = false,
    claude_backup' = false,
    legacy_claude_local' = false,
    codex_present' = true,
    codex_patched' = false,
    codex_backup' = false,
    copilot_cli_present' = true,
    copilot_cli_patched' = false,
    copilot_cli_backup' = false,
    openclaw_present' = true,
    openclaw_patched' = false,
    openclaw_backup' = false,
    vibe_present' = true,
    vibe_patched' = false,
    vibe_backup' = false,
    skill_ready' = false
  }

  action seed_ready_workspace: bool = all {
    config_ready' = true,
    cursor_present' = true,
    cursor_patched' = true,
    cursor_backup' = true,
    legacy_cursor_local' = false,
    claude_present' = true,
    claude_patched' = true,
    claude_backup' = true,
    legacy_claude_local' = false,
    codex_present' = true,
    codex_patched' = true,
    codex_backup' = true,
    copilot_cli_present' = true,
    copilot_cli_patched' = true,
    copilot_cli_backup' = true,
    openclaw_present' = true,
    openclaw_patched' = true,
    openclaw_backup' = true,
    vibe_present' = true,
    vibe_patched' = true,
    vibe_backup' = true,
    skill_ready' = true
  }

  // Represents a workspace previously init'd with local wiring (old kaizen version).
  // Global wiring is already present; workspace-local files also contain kaizen hooks.
  action seed_legacy_local_wiring: bool = all {
    config_ready' = true,
    cursor_present' = true,
    cursor_patched' = true,
    cursor_backup' = true,
    legacy_cursor_local' = true,
    claude_present' = true,
    claude_patched' = true,
    claude_backup' = true,
    legacy_claude_local' = true,
    codex_present' = true,
    codex_patched' = true,
    codex_backup' = true,
    copilot_cli_present' = true,
    copilot_cli_patched' = true,
    copilot_cli_backup' = true,
    openclaw_present' = true,
    openclaw_patched' = true,
    openclaw_backup' = true,
    vibe_present' = false,
    vibe_patched' = false,
    vibe_backup' = false,
    skill_ready' = true
  }

  // `kaizen init` scaffolds hook config when absent — workspaces become
  // instrumented for all five hosts in one command. Existing files backed up.
  // Since v0.2: wiring target is user-global (~/.cursor/, ~/.claude/); legacy
  // local wiring is preserved (not removed) — init is still idempotent.
  action run_init: bool = all {
    config_ready' = true,
    cursor_present' = true,
    cursor_patched' = true,
    cursor_backup' = (cursor_backup or (cursor_present and not(cursor_patched))),
    legacy_cursor_local' = legacy_cursor_local,
    claude_present' = true,
    claude_patched' = true,
    claude_backup' = (claude_backup or (claude_present and not(claude_patched))),
    legacy_claude_local' = legacy_claude_local,
    codex_present' = true,
    codex_patched' = true,
    codex_backup' = (codex_backup or (codex_present and not(codex_patched))),
    copilot_cli_present' = true,
    copilot_cli_patched' = true,
    copilot_cli_backup' = (copilot_cli_backup or (copilot_cli_present and not(copilot_cli_patched))),
    openclaw_present' = true,
    openclaw_patched' = true,
    openclaw_backup' = (openclaw_backup or (openclaw_present and not(openclaw_patched))),
    vibe_present' = true,
    vibe_patched' = true,
    vibe_backup' = (vibe_backup or (vibe_present and not(vibe_patched))),
    skill_ready' = true
  }

  action step: bool = any {
    seed_empty_workspace,
    seed_existing_hook_files,
    seed_ready_workspace,
    seed_legacy_local_wiring,
    run_init
  }

  val fully_ready: bool =
    all {
      config_ready,
      cursor_present, cursor_patched, cursor_backup,
      claude_present, claude_patched, claude_backup,
      codex_present, codex_patched, codex_backup,
      copilot_cli_present, copilot_cli_patched, copilot_cli_backup,
      openclaw_present, openclaw_patched, openclaw_backup,
      vibe_present, vibe_patched, vibe_backup,
      skill_ready
    }

  val cursor_backup_requires_patch: bool =
    cursor_backup implies (cursor_present and cursor_patched)

  val claude_backup_requires_patch: bool =
    claude_backup implies (claude_present and claude_patched)

  val codex_backup_requires_patch: bool =
    codex_backup implies (codex_present and codex_patched)

  val copilot_cli_backup_requires_patch: bool =
    copilot_cli_backup implies (copilot_cli_present and copilot_cli_patched)

  val openclaw_backup_requires_patch: bool =
    openclaw_backup implies (openclaw_present and openclaw_patched)

  val vibe_backup_requires_patch: bool =
    vibe_backup implies (vibe_present and vibe_patched)

  // Legacy local wiring can only exist when the global wiring is also in place.
  val legacy_cursor_implies_patched: bool =
    legacy_cursor_local implies cursor_patched

  val legacy_claude_implies_patched: bool =
    legacy_claude_local implies claude_patched

  run initCreatesMissingArtifacts = {
    seed_empty_workspace
      .then(run_init)
      .expect(all {
        config_ready,
        skill_ready,
        not(cursor_backup),
        not(claude_backup),
        not(codex_backup),
        not(copilot_cli_backup),
        not(openclaw_backup),
        not(vibe_backup),
      })
  }

  run initPatchesExistingHookFiles = {
    seed_existing_hook_files
      .then(run_init)
      .expect(all {
        cursor_patched, cursor_backup,
        claude_patched, claude_backup,
        codex_patched, codex_backup,
        copilot_cli_patched, copilot_cli_backup,
        openclaw_patched, openclaw_backup,
        vibe_patched, vibe_backup,
      })
  }

  run rerunIsNoop = {
    seed_ready_workspace
      .then(run_init)
      .expect(fully_ready)
  }

  // Legacy local wiring is preserved (not removed) after a global init rerun.
  run legacyWiringPreservedAfterGlobalInit = {
    seed_legacy_local_wiring
      .then(run_init)
      .expect(all {
        cursor_patched, legacy_cursor_local,
        claude_patched, legacy_claude_local,
      })
  }

  // A fresh workspace has no legacy local wiring after init.
  run freshInitHasNoLegacy = {
    seed_empty_workspace
      .then(run_init)
      .expect(all {
        cursor_patched, not(legacy_cursor_local),
        claude_patched, not(legacy_claude_local),
      })
  }
}
