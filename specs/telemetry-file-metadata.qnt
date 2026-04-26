// Local NDJSON file exporter: one summary JSON line per successful export when the file sink is on.
// Metadata: batch_kind + item_count + kaizen_schema_version (conceptual); team_id + workspace_hash
// only for ingest bodies that include an envelope (events, tool_spans, repo_snapshots, workspace_facts).
// session_evals / session_feedback: items-only batch bodies — metadata omits team/workspace (v1).
// Tail follow mode never delivers lines before they exist: delivered line count <= lines on disk.
module telemetry_file_metadata {
  var ndjson_lines: int
  var file_sink_enabled: bool
  // Lines already observed by a tail -f client (abstract).
  var tail_delivered: int
  var last_includes_envelope: bool
  var last_batch_kind: str

  action init: bool = all {
    ndjson_lines' = 0,
    file_sink_enabled' = false,
    tail_delivered' = 0,
    last_includes_envelope' = false,
    last_batch_kind' = "",
  }

  action enable_file_sink: bool = all {
    file_sink_enabled' = true,
    ndjson_lines' = ndjson_lines,
    tail_delivered' = tail_delivered,
    last_includes_envelope' = last_includes_envelope,
    last_batch_kind' = last_batch_kind,
  }

  // Envelope in ingest body → metadata may list team + workspace.
  action export_enveloped: bool = {
    nondet k = oneOf(Set("events", "tool_spans", "repo_snapshots", "workspace_facts"))
    all {
      file_sink_enabled,
      file_sink_enabled' = file_sink_enabled,
      ndjson_lines' = ndjson_lines + 1,
      last_includes_envelope' = true,
      last_batch_kind' = k,
      tail_delivered' = tail_delivered,
    }
  }

  // Items-only batch bodies: no team/workspace in metadata (v1).
  action export_body_only: bool = {
    nondet k = oneOf(Set("session_evals", "session_feedback"))
    all {
      file_sink_enabled,
      file_sink_enabled' = file_sink_enabled,
      ndjson_lines' = ndjson_lines + 1,
      last_includes_envelope' = false,
      last_batch_kind' = k,
      tail_delivered' = tail_delivered,
    }
  }

  action export_file_disabled: bool = all {
    not(file_sink_enabled),
    file_sink_enabled' = file_sink_enabled,
    ndjson_lines' = ndjson_lines,
    tail_delivered' = tail_delivered,
    last_includes_envelope' = last_includes_envelope,
    last_batch_kind' = last_batch_kind,
  }

  // tail -f catches up to EOF: deliver all complete lines so far.
  action tail_catchup: bool = all {
    tail_delivered' = ndjson_lines,
    ndjson_lines' = ndjson_lines,
    file_sink_enabled' = file_sink_enabled,
    last_includes_envelope' = last_includes_envelope,
    last_batch_kind' = last_batch_kind,
  }

  val nonneg: bool = ndjson_lines >= 0

  val tail_not_ahead: bool = tail_delivered <= ndjson_lines

  // session_* batches never carry team/workspace in file metadata in v1.
  val body_only_no_envelope: bool =
    (last_batch_kind == "session_evals" or last_batch_kind == "session_feedback")
      implies not(last_includes_envelope)

  val enveloped_kinds: bool = (not(last_batch_kind == "session_evals")
    and not(last_batch_kind == "session_feedback") and not(last_batch_kind == "")) implies
    last_includes_envelope

  run kind_and_tail = {
    init
      .then(enable_file_sink)
      .then(export_enveloped)
      .then(export_body_only)
      .then(tail_catchup)
      .expect(ndjson_lines == 2 and tail_delivered == 2)
  }

  run file_off_no_lines = {
    init.then(export_file_disabled).expect(ndjson_lines == 0)
  }

  // For `quint-connect` random walks (see tests/spec/telemetry_file_metadata.rs).
  action step: bool = any {
    enable_file_sink,
    export_enveloped,
    export_body_only,
    export_file_disabled,
    tail_catchup,
  }
}
