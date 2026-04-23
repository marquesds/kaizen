// `kaizen doctor`: ordered checks; config or store hard-failure reaches Terminal with failed=true.
module doctor_diagnostic {
  var phase: str
  var failed: bool

  action init: bool = all {
    phase' = "Start",
    failed' = false
  }

  action check_config_ok: bool = all {
    phase == "Start",
    not(failed),
    phase' = "AfterConfig",
    failed' = false
  }

  action check_config_err: bool = all {
    phase == "Start",
    phase' = "Terminal",
    failed' = true
  }

  action check_store_ok: bool = all {
    phase == "AfterConfig",
    not(failed),
    phase' = "AfterStore",
    failed' = false
  }

  action check_store_err: bool = all {
    phase == "AfterConfig",
    phase' = "Terminal",
    failed' = true
  }

  action finish_hooks: bool = all {
    phase == "AfterStore",
    not(failed),
    phase' = "Done",
    failed' = false
  }

  action step: bool = any {
    check_config_ok,
    check_config_err,
    check_store_ok,
    check_store_err,
    finish_hooks
  }

  val terminal_implies_failed: bool =
    (phase == "Terminal") implies failed

  val done_implies_ok: bool =
    (phase == "Done") implies not(failed)

  run happyPath = {
    init
      .then(check_config_ok)
      .then(check_store_ok)
      .then(finish_hooks)
      .expect(all { phase == "Done", not(failed) })
  }
}
