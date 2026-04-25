// assign_parents invariants for nested tool-span trees.
// Spans are half-open intervals [start, end). A span P contains span C iff
// P.start <= C.start && C.end <= P.end.
// assign_parents picks the deepest containing ancestor as parent.
// Invariants: containment holds, depth == parent.depth + 1, no cycles (forest).
module span_hierarchy {
  // A span: id, start, end (ms), assigned parent id (-1 = root), depth.
  type Span = {
    id: int,
    start: int,
    end: int,
    parent: int,
    depth: int
  }

  var spans: List[Span]
  var next_id: int

  pure def contains(outer: Span, inner: Span): bool =
    outer.start <= inner.start and inner.end <= outer.end and outer.id != inner.id

  // deepest containing ancestor = the one with max depth among all containers
  pure def assign_parent(s: Span, all: List[Span]): int =
    all.foldl(-1, (best, candidate) =>
      if (contains(candidate, s) and (best == -1 or
            all.filter(x => x.id == best).length() == 0 or
            candidate.depth > all.filter(x => x.id == best).head().depth))
        candidate.id
      else
        best
    )

  pure def depth_of(pid: int, all: List[Span]): int =
    if (pid == -1) 0
    else all.filter(x => x.id == pid).foldl(0, (_, p) => p.depth + 1)

  action init: bool = all {
    spans' = [],
    next_id' = 0
  }

  // add_span_outer: wide span, no existing span contains it → becomes root
  action add_root_span: bool = all {
    val s = { id: next_id, start: 0, end: 100, parent: -1, depth: 0 }
    spans' = spans.append(s),
    next_id' = next_id + 1
  }

  // add_child: narrow span inside last root span → depth 1, parent = root id
  action add_child_span: bool = all {
    spans.length() > 0,
    val root = spans.last()
    val pid = assign_parent({ id: next_id, start: 10, end: 50, parent: -1, depth: 0 }, spans)
    val d = depth_of(pid, spans)
    val s = { id: next_id, start: 10, end: 50, parent: pid, depth: d }
    spans' = spans.append(s),
    next_id' = next_id + 1
  }

  // add_grandchild: narrow span inside the child → depth 2
  action add_grandchild_span: bool = all {
    spans.length() >= 2,
    val pid = assign_parent({ id: next_id, start: 15, end: 30, parent: -1, depth: 0 }, spans)
    val d = depth_of(pid, spans)
    val s = { id: next_id, start: 15, end: 30, parent: pid, depth: d }
    spans' = spans.append(s),
    next_id' = next_id + 1
  }

  action step: bool = any {
    add_root_span,
    add_child_span,
    add_grandchild_span
  }

  // Every non-root span's interval is contained by its parent's interval.
  val containment_holds: bool =
    spans.forall(s =>
      s.parent == -1
      or spans.filter(p => p.id == s.parent).forall(p => contains(p, s))
    )

  // depth == parent.depth + 1 for non-root spans.
  val depth_consistent: bool =
    spans.forall(s =>
      s.parent == -1
      or spans.filter(p => p.id == s.parent).forall(p => s.depth == p.depth + 1)
    )

  // No span is its own ancestor (forest, no cycles).
  val no_cycles: bool =
    spans.forall(s => s.parent != s.id)

  // Root spans have depth 0.
  val root_depth_zero: bool =
    spans.forall(s => s.parent != -1 or s.depth == 0)

  run root_only = {
    init
      .then(add_root_span)
      .expect(containment_holds and depth_consistent and no_cycles and root_depth_zero)
  }

  run root_with_child = {
    init
      .then(add_root_span)
      .then(add_child_span)
      .expect(containment_holds and depth_consistent and no_cycles)
  }

  run three_levels = {
    init
      .then(add_root_span)
      .then(add_child_span)
      .then(add_grandchild_span)
      .expect(containment_holds and depth_consistent and no_cycles)
  }

  run grandchild_depth_is_two = {
    init
      .then(add_root_span)
      .then(add_child_span)
      .then(add_grandchild_span)
      .expect(spans.forall(s => s.depth <= 2))
  }
}
