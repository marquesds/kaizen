// `kaizen sessions tree <id>` render contract.

module session_tree {
  var phase: str
  var session_exists: bool
  var span_count: int
  var rendered: str

  action init: bool = all {
    phase' = "Start",
    session_exists' = false,
    span_count' = 0,
    rendered' = "none"
  }

  action lookup_missing: bool = all {
    phase == "Start",
    phase' = "Missing",
    session_exists' = false,
    span_count' = 0,
    rendered' = rendered
  }

  action lookup_empty: bool = all {
    phase == "Start",
    phase' = "Ready",
    session_exists' = true,
    span_count' = 0,
    rendered' = rendered
  }

  action lookup_spans: bool = all {
    phase == "Start",
    phase' = "Ready",
    session_exists' = true,
    span_count' = 1,
    rendered' = rendered
  }

  action render_text_missing: bool = all {
    phase == "Missing",
    phase' = "Error",
    session_exists' = session_exists,
    span_count' = span_count,
    rendered' = "error"
  }

  action render_text_empty: bool = all {
    phase == "Ready",
    session_exists,
    span_count == 0,
    phase' = "Rendered",
    session_exists' = session_exists,
    span_count' = span_count,
    rendered' = "placeholder"
  }

  action render_text_tree: bool = all {
    phase == "Ready",
    session_exists,
    span_count > 0,
    phase' = "Rendered",
    session_exists' = session_exists,
    span_count' = span_count,
    rendered' = "tree"
  }

  action render_json_empty: bool = all {
    phase == "Ready",
    session_exists,
    span_count == 0,
    phase' = "Rendered",
    session_exists' = session_exists,
    span_count' = span_count,
    rendered' = "json_empty"
  }

  action render_json_tree: bool = all {
    phase == "Ready",
    session_exists,
    span_count > 0,
    phase' = "Rendered",
    session_exists' = session_exists,
    span_count' = span_count,
    rendered' = "json_tree"
  }

  action step: bool = any {
    lookup_missing,
    lookup_empty,
    lookup_spans,
    render_text_missing,
    render_text_empty,
    render_text_tree,
    render_json_empty,
    render_json_tree
  }

  val no_successful_render_without_session: bool =
    (rendered == "placeholder" or rendered == "tree" or rendered == "json_empty" or rendered == "json_tree") implies session_exists
  val text_empty_uses_placeholder: bool =
    (phase == "Rendered" and span_count == 0 and rendered == "placeholder") implies session_exists
  val missing_errors: bool =
    (phase == "Error") implies rendered == "error"

  run empty_text_renders_placeholder = {
    init.then(lookup_empty).then(render_text_empty)
      .expect(rendered == "placeholder")
  }

  run empty_json_stays_empty_array = {
    init.then(lookup_empty).then(render_json_empty)
      .expect(rendered == "json_empty")
  }

  run missing_session_errors = {
    init.then(lookup_missing).then(render_text_missing)
      .expect(missing_errors)
  }
}
