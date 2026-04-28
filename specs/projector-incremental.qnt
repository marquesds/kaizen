// Incremental projector state machine.
// Open spans stay memory-only. Closed/orphaned spans become persisted rows.

module projector_incremental {
  type Span = {
    id: int,
    start: int,
    end: int,
    status: str
  }

  var open: Set[int]
  var rows: Set[Span]

  action init: bool = all {
    open' = Set(),
    rows' = Set()
  }

  action open_span: bool = all {
    open' = open.union(Set(1)),
    rows' = rows
  }

  action close_span: bool = all {
    open.contains(1),
    open' = open.exclude(Set(1)),
    rows' = rows.union(Set({ id: 1, start: 0, end: 10, status: "done" }))
  }

  action orphan_span: bool = all {
    open.contains(1),
    open' = open.exclude(Set(1)),
    rows' = rows.union(Set({ id: 1, start: 0, end: 0, status: "orphaned" }))
  }

  action step: bool = any {
    open_span,
    close_span,
    orphan_span
  }

  val rows_are_terminal: bool =
    rows.forall(row => row.status == "done" or row.status == "orphaned")

  val open_not_persisted: bool =
    open.contains(1) implies rows.forall(row => row.id != 1)

  run close_refines_terminal_row = {
    init
      .then(open_span)
      .then(close_span)
      .expect(rows_are_terminal)
  }

  run orphan_refines_terminal_row = {
    init
      .then(open_span)
      .then(orphan_span)
      .expect(rows_are_terminal)
  }

  run open_span_memory_only = {
    init
      .then(open_span)
      .expect(open_not_persisted)
  }
}
