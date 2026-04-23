// `kaizen sessions show <id>`: open store, lookup, render only on hit.
module session_lookup {
  var phase: str
  var found: bool

  action init: bool = all {
    phase' = "Start",
    found' = false
  }

  action open_store: bool = all {
    phase == "Start",
    phase' = "Open",
    found' = false
  }

  action lookup_hit: bool = all {
    phase == "Open",
    phase' = "Ready",
    found' = true
  }

  action lookup_miss: bool = all {
    phase == "Open",
    phase' = "Missing",
    found' = false
  }

  action render: bool = all {
    phase == "Ready",
    phase' = "Done",
    found' = found
  }

  action step: bool = any {
    open_store,
    lookup_hit,
    lookup_miss,
    render
  }

  val missing_not_found: bool =
    (phase == "Missing") implies not(found)

  val done_implies_found: bool =
    (phase == "Done") implies found

  run show_hit = {
    init
      .then(open_store)
      .then(lookup_hit)
      .then(render)
      .expect(all { phase == "Done", found })
  }
}
