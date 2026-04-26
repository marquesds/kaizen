// H33 — automation opportunity: same-tool runs and repeating tool subsequences.
// Rust: `src/retro/heuristics/h33/`. Gates and token scalars mirror constants there.
module h33_automation {
  val MIN_RUN_LEN: int = 5
  val MIN_SESSIONS_WITH_RUN: int = 1
  val DOUBLE_RUN_LEN: int = 10
  val TOKENS_PER_EXTRA_RUN_CALL: int = 150

  val MIN_SUBSEQ_REPEATS_LEN2: int = 3
  val MIN_SUBSEQ_REPEATS_LEN3: int = 2
  val TOKENS_PER_SUBSEQ_CYCLE: int = 200

  var distinct_qualifying_sessions: int
  var max_qualifying_run_len: int

  // Emit a same-tool run bet when any qualifying run exists and this gate holds.
  pure def run_bet_gate(distinct_sessions_with_qualifying: int, max_run_len: int): bool =
    distinct_sessions_with_qualifying >= MIN_SESSIONS_WITH_RUN
    or max_run_len >= DOUBLE_RUN_LEN

  pure def tokens_saved_runs(extra_calls_sum: int): int =
    TOKENS_PER_EXTRA_RUN_CALL * extra_calls_sum

  pure def subseq_qualifies(repeat_count: int, k: int): bool =
    if (k == 2) repeat_count >= MIN_SUBSEQ_REPEATS_LEN2
    else if (k == 3) repeat_count >= MIN_SUBSEQ_REPEATS_LEN3
    else false

  pure def tokens_saved_subseq(total_repeats: int, mult_num: int, mult_den: int): int =
    (TOKENS_PER_SUBSEQ_CYCLE * total_repeats * mult_num) / mult_den

  action init: bool = all {
    distinct_qualifying_sessions' = 0,
    max_qualifying_run_len' = 0,
  }

  action set_one_session_run5: bool = all {
    distinct_qualifying_sessions' = 1,
    max_qualifying_run_len' = 5,
  }

  action set_one_session_run10: bool = all {
    distinct_qualifying_sessions' = 1,
    max_qualifying_run_len' = 10,
  }

  action set_two_sessions_run5: bool = all {
    distinct_qualifying_sessions' = 2,
    max_qualifying_run_len' = 5,
  }

  action set_no_qualifying: bool = all {
    distinct_qualifying_sessions' = 0,
    max_qualifying_run_len' = 4,
  }

  action step: bool = any {
    set_one_session_run5,
    set_one_session_run10,
    set_two_sessions_run5,
    set_no_qualifying,
  }

  val gate_matches_state: bool =
    run_bet_gate(distinct_qualifying_sessions, max_qualifying_run_len) ==
      (distinct_qualifying_sessions >= MIN_SESSIONS_WITH_RUN
        or max_qualifying_run_len >= DOUBLE_RUN_LEN)

  val subseq_len2_ok: bool = subseq_qualifies(3, 2)
  val subseq_len2_short: bool = not(subseq_qualifies(2, 2))
  val subseq_len3_ok: bool = subseq_qualifies(2, 3)

  run one_session_five_emits = {
    init
      .then(set_one_session_run5)
      .expect(
        run_bet_gate(distinct_qualifying_sessions, max_qualifying_run_len)
          and tokens_saved_runs(4) == 600
      )
  }

  run double_run_threshold_emits = {
    init
      .then(set_one_session_run10)
      .expect(run_bet_gate(distinct_qualifying_sessions, max_qualifying_run_len))
  }

  run two_distinct_sessions_emits = {
    init
      .then(set_two_sessions_run5)
      .expect(run_bet_gate(distinct_qualifying_sessions, max_qualifying_run_len))
  }

  run weak_signal_no_gate = {
    init
      .then(set_no_qualifying)
      .expect(not(run_bet_gate(distinct_qualifying_sessions, max_qualifying_run_len)))
  }

  run subseq_token_bonus = {
    init.expect(tokens_saved_subseq(3, 5, 4) == 750)
  }
}
