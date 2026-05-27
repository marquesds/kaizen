// Optional LLM guidance proposal gate.
// LLM proposals require explicit CLI flag plus enabled config.
// Redaction, max-op cap, and rejected-candidate memory must survive generation.

module guidance_proposal_llm {
  var config_enabled: bool
  var llm_flag: bool
  var redact_config: bool
  var raw_input_seen: bool
  var sent_raw_input: bool
  var rejected_memory_seen: bool
  var sent_rejected_memory: bool
  var requested_ops: int
  var emitted_ops: int
  var candidate_generated: bool
  var error: bool

  pure def cap_ops(n: int): int = if (n > 3) 3 else n

  action init: bool = all {
    config_enabled' = false,
    llm_flag' = false,
    redact_config' = true,
    raw_input_seen' = false,
    sent_raw_input' = false,
    rejected_memory_seen' = false,
    sent_rejected_memory' = false,
    requested_ops' = 0,
    emitted_ops' = 0,
    candidate_generated' = false,
    error' = false
  }

  action enable_config: bool = all {
    config_enabled' = true,
    llm_flag' = llm_flag,
    redact_config' = redact_config,
    raw_input_seen' = raw_input_seen,
    sent_raw_input' = sent_raw_input,
    rejected_memory_seen' = rejected_memory_seen,
    sent_rejected_memory' = sent_rejected_memory,
    requested_ops' = requested_ops,
    emitted_ops' = emitted_ops,
    candidate_generated' = candidate_generated,
    error' = error
  }

  action request_llm: bool = all {
    config_enabled' = config_enabled,
    llm_flag' = true,
    redact_config' = redact_config,
    raw_input_seen' = raw_input_seen,
    sent_raw_input' = sent_raw_input,
    rejected_memory_seen' = rejected_memory_seen,
    sent_rejected_memory' = sent_rejected_memory,
    requested_ops' = requested_ops,
    emitted_ops' = emitted_ops,
    candidate_generated' = candidate_generated,
    error' = error
  }

  action attach_inputs: bool = all {
    config_enabled' = config_enabled,
    llm_flag' = llm_flag,
    redact_config' = redact_config,
    raw_input_seen' = true,
    sent_raw_input' = sent_raw_input,
    rejected_memory_seen' = true,
    sent_rejected_memory' = sent_rejected_memory,
    requested_ops' = 5,
    emitted_ops' = emitted_ops,
    candidate_generated' = candidate_generated,
    error' = error
  }

  action disable_redaction: bool = all {
    config_enabled' = config_enabled,
    llm_flag' = llm_flag,
    redact_config' = false,
    raw_input_seen' = raw_input_seen,
    sent_raw_input' = sent_raw_input,
    rejected_memory_seen' = rejected_memory_seen,
    sent_rejected_memory' = sent_rejected_memory,
    requested_ops' = requested_ops,
    emitted_ops' = emitted_ops,
    candidate_generated' = candidate_generated,
    error' = error
  }

  action generate: bool = all {
    config_enabled' = config_enabled,
    llm_flag' = llm_flag,
    redact_config' = redact_config,
    raw_input_seen' = raw_input_seen,
    sent_raw_input' = raw_input_seen and not(redact_config),
    rejected_memory_seen' = rejected_memory_seen,
    sent_rejected_memory' = rejected_memory_seen,
    requested_ops' = requested_ops,
    emitted_ops' = if (config_enabled and llm_flag) cap_ops(requested_ops) else 0,
    candidate_generated' = config_enabled and llm_flag,
    error' = not(config_enabled and llm_flag)
  }

  action step: bool = any { enable_config, request_llm, attach_inputs, disable_redaction, generate }

  val generated_requires_double_gate: bool =
    candidate_generated implies (config_enabled and llm_flag)
  val max_ops_enforced: bool = emitted_ops <= 3
  val redaction_blocks_raw: bool =
    (candidate_generated and raw_input_seen and redact_config) implies not(sent_raw_input)
  val rejected_memory_forwarded: bool =
    (candidate_generated and rejected_memory_seen) implies sent_rejected_memory

  run disabled_blocks_generation = {
    init.then(request_llm).then(attach_inputs).then(generate)
      .expect(error and not(candidate_generated))
  }

  run redacted_generation_caps_ops = {
    init.then(enable_config).then(request_llm).then(attach_inputs).then(generate)
      .expect(candidate_generated and emitted_ops == 3 and not(sent_raw_input) and sent_rejected_memory)
  }

  run unredacted_generation_may_send_raw = {
    init.then(enable_config).then(request_llm).then(disable_redaction).then(attach_inputs).then(generate)
      .expect(candidate_generated and sent_raw_input)
  }
}
