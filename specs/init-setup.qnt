// `kaizen init`: install config + skill, patch hook files once, rerun is noop.
module init_setup {
  var config_ready: bool
  var cursor_present: bool
  var cursor_patched: bool
  var cursor_backup: bool
  var claude_present: bool
  var claude_patched: bool
  var claude_backup: bool
  var skill_ready: bool

  action init: bool = all {
    config_ready' = false,
    cursor_present' = false,
    cursor_patched' = false,
    cursor_backup' = false,
    claude_present' = false,
    claude_patched' = false,
    claude_backup' = false,
    skill_ready' = false
  }

  action seed_empty_workspace: bool = init

  action seed_existing_hook_files: bool = all {
    config_ready' = false,
    cursor_present' = true,
    cursor_patched' = false,
    cursor_backup' = false,
    claude_present' = true,
    claude_patched' = false,
    claude_backup' = false,
    skill_ready' = false
  }

  action seed_ready_workspace: bool = all {
    config_ready' = true,
    cursor_present' = true,
    cursor_patched' = true,
    cursor_backup' = true,
    claude_present' = true,
    claude_patched' = true,
    claude_backup' = true,
    skill_ready' = true
  }

  action run_init: bool = all {
    config_ready' = true,
    cursor_present' = cursor_present,
    cursor_patched' = (cursor_patched or cursor_present),
    cursor_backup' = (cursor_backup or (cursor_present and not(cursor_patched))),
    claude_present' = claude_present,
    claude_patched' = (claude_patched or claude_present),
    claude_backup' = (claude_backup or (claude_present and not(claude_patched))),
    skill_ready' = true
  }

  action step: bool = any {
    seed_empty_workspace,
    seed_existing_hook_files,
    seed_ready_workspace,
    run_init
  }

  val fully_ready: bool =
    all {
      config_ready,
      cursor_present,
      cursor_patched,
      cursor_backup,
      claude_present,
      claude_patched,
      claude_backup,
      skill_ready
    }

  val cursor_backup_requires_patch: bool =
    cursor_backup implies (cursor_present and cursor_patched)

  val claude_backup_requires_patch: bool =
    claude_backup implies (claude_present and claude_patched)

  run initCreatesMissingArtifacts = {
    seed_empty_workspace
      .then(run_init)
      .expect(all { config_ready, skill_ready, not(cursor_backup), not(claude_backup) })
  }

  run initPatchesExistingHookFiles = {
    seed_existing_hook_files
      .then(run_init)
      .expect(all { cursor_patched, cursor_backup, claude_patched, claude_backup })
  }

  run rerunIsNoop = {
    seed_ready_workspace
      .then(run_init)
      .expect(fully_ready)
  }
}
