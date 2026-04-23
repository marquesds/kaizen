// `kaizen` self-update: registry check, optional user gate, at-most-one install, integrity.
// v1 abstracts registry + cargo as ordered `local` / `remote` ints; real impl: crates.io tags or
// `cargo install` target channel.
module auto_update {
  // 0=idle, 1=check in flight, 2=install in flight
  var op: int
  var local: int
  var remote: int
  // 0 = not fetched; positive = last fetch result
  var auto_install: bool
  // user ok for this candidate (manual path); cleared after use
  var approved: bool
  // artifact passed sig/hash gate before `replacing` (impl: crate checksum or attestation)
  var integrity_ok: bool

  action init: bool = all {
    op' = 0,
    local' = 1,
    remote' = 0,
    auto_install' = false,
    approved' = false,
    integrity_ok' = false
  }

  action enable_auto: bool = all {
    op' = op,
    local' = local,
    remote' = remote,
    auto_install' = true,
    approved' = approved,
    integrity_ok' = integrity_ok
  }

  action user_approves: bool = all {
    op' = op,
    local' = local,
    remote' = remote,
    auto_install' = auto_install,
    approved' = true,
    integrity_ok' = integrity_ok
  }

  // Start background check (e.g. `kaizen update --check` or scheduler tick)
  action begin_check: bool = all {
    op == 0,
    op' = 1,
    local' = local,
    remote' = remote,
    auto_install' = auto_install,
    approved' = approved,
    integrity_ok' = integrity_ok
  }

  // Registry responded: up to date (canonical version = local; fetch may not have known it before)
  action check_no_update: bool = all {
    op == 1,
    op' = 0,
    local' = local,
    remote' = local,
    auto_install' = auto_install,
    approved' = approved,
    integrity_ok' = integrity_ok
  }

  // Registry responded: newer available
  action check_update_available: bool = all {
    op == 1,
    op' = 0,
    local' = local,
    remote' = local + 1,
    auto_install' = auto_install,
    approved' = approved,
    integrity_ok' = false
  }

  // Network/parse error: remote stays unknown, no install path
  action check_fail: bool = all {
    op == 1,
    op' = 0,
    local' = local,
    remote' = 0,
    auto_install' = auto_install,
    approved' = approved,
    integrity_ok' = integrity_ok
  }

  // After download: verify (signature/hash) before touching binary
  action verify_ok: bool = all {
    op == 0,
    remote > 0,
    remote > local,
    op' = 0,
    local' = local,
    remote' = remote,
    auto_install' = auto_install,
    approved' = approved,
    integrity_ok' = true
  }

  action verify_fail: bool = all {
    op == 0,
    remote > local,
    op' = 0,
    local' = local,
    remote' = remote,
    auto_install' = auto_install,
    approved' = approved,
    integrity_ok' = false
  }

  // Replace running binary: requires integrity (or same-version noop not modeled here)
  action begin_install: bool = all {
    op == 0,
    remote > local,
    integrity_ok,
    (auto_install or approved),
    op' = 2,
    local' = local,
    remote' = remote,
    auto_install' = auto_install,
    approved' = approved,
    integrity_ok' = integrity_ok
  }

  action install_ok: bool = all {
    op == 2,
    op' = 0,
    local' = remote,
    remote' = remote,
    auto_install' = auto_install,
    approved' = false,
    integrity_ok' = false
  }

  // Failed mid-install: local unchanged, stop pipeline
  action install_fail: bool = all {
    op == 2,
    op' = 0,
    local' = local,
    remote' = remote,
    auto_install' = auto_install,
    approved' = false,
    integrity_ok' = false
  }

  action step: bool = any {
    init,
    enable_auto,
    user_approves,
    begin_check,
    check_no_update,
    check_update_available,
    check_fail,
    verify_ok,
    verify_fail,
    begin_install,
    install_ok,
    install_fail
  }

  val op_range: bool = (op == 0 or op == 1 or op == 2)
  // While installing, we already proved candidate integrity at begin_install
  val op_implies_invariants: bool =
    (op == 0 or op == 1 or (op == 2 and remote > local and integrity_ok and (auto_install or approved)))

  // Gating: no install without newer remote, trust, and policy
  val no_install_untrusted: bool =
    (op == 2) implies (remote > local and integrity_ok and (auto_install or approved))

  run happyAutoPath = {
    init
      .then(enable_auto)
      .then(begin_check)
      .then(check_update_available)
      .then(verify_ok)
      .then(begin_install)
      .then(install_ok)
      .expect(op == 0 and local == remote and remote == 2)
  }

  run happyManualPath = {
    init
      .then(begin_check)
      .then(check_update_available)
      .then(verify_ok)
      .then(user_approves)
      .then(begin_install)
      .then(install_ok)
      .expect(op == 0 and local == 2)
  }

  run alreadyLatest = {
    init
      .then(begin_check)
      .then(check_no_update)
      .expect(op == 0 and local == remote and remote >= local)
  }

  run checkFailsRemoteUnknown = {
    init
      .then(begin_check)
      .then(check_fail)
      .expect(remote == 0 and op == 0)
  }

  run verifyBlocksInstall = {
    init
      .then(enable_auto)
      .then(begin_check)
      .then(check_update_available)
      // integrity_ok false: begin_install should be unreachable from driver if spec is tight;
      // model shows user cannot proceed without verify_ok
      .expect(op == 0 and remote > local and not(integrity_ok))
  }
}
