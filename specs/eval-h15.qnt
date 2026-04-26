// H15 trigger invariants for LLM-as-a-Judge evaluations.
// Scores are integers 0..100 (mapping from 0.0..1.0; "low" = score < 40).
// H15 fires when: n_low >= 3 OR (prior_mean > 0 AND current mean dropped > 15 pts vs prior).
// h15_fired is sticky within a window; rotation resets it.
module eval_h15 {
  var n_evals: int
  var n_low: int
  var sum_scores: int
  var prior_mean: int
  var h15_fired: bool

  pure def trigger(n_l: int, n_e: int, s: int, pm: int): bool =
    n_l >= 3
    or (n_e > 0 and pm > 0 and (pm * n_e - s) > 15 * n_e)

  action init: bool = all {
    n_evals' = 0,
    n_low' = 0,
    sum_scores' = 0,
    prior_mean' = 0,
    h15_fired' = false
  }

  action add_low: bool = all {
    n_evals' = n_evals + 1,
    n_low' = n_low + 1,
    sum_scores' = sum_scores + 20,
    prior_mean' = prior_mean,
    h15_fired' = if (h15_fired) true else trigger(n_low + 1, n_evals + 1, sum_scores + 20, prior_mean)
  }

  action add_ok: bool = all {
    n_evals' = n_evals + 1,
    n_low' = n_low,
    sum_scores' = sum_scores + 70,
    prior_mean' = prior_mean,
    h15_fired' = if (h15_fired) true else trigger(n_low, n_evals + 1, sum_scores + 70, prior_mean)
  }

  action rotate: bool = all {
    n_evals > 0,
    n_evals' = 0,
    n_low' = 0,
    sum_scores' = 0,
    prior_mean' = sum_scores / n_evals,
    h15_fired' = false
  }

  action step: bool = any {
    add_low,
    add_ok,
    rotate
  }

  val low_le_total: bool = n_low <= n_evals
  val counts_non_negative: bool =
    n_evals >= 0 and n_low >= 0 and sum_scores >= 0
  val prior_mean_valid: bool = prior_mean >= 0 and prior_mean <= 100
  val no_fire_empty: bool = (n_evals == 0) implies not(h15_fired)

  run three_low_fires = {
    init
      .then(add_low)
      .then(add_low)
      .then(add_low)
      .expect(h15_fired)
  }

  run two_low_no_fire = {
    init
      .then(add_low)
      .then(add_low)
      .expect(not(h15_fired))
  }

  run mean_drop_fires = {
    init
      .then(add_ok)
      .then(add_ok)
      .then(rotate)
      .then(add_low)
      .expect(h15_fired)
  }

  run h15_sticky = {
    init
      .then(add_low)
      .then(add_low)
      .then(add_low)
      .then(add_ok)
      .expect(h15_fired)
  }

  run rotation_resets = {
    init
      .then(add_low)
      .then(add_low)
      .then(add_low)
      .then(rotate)
      .expect(not(h15_fired))
  }

  run two_good_windows_no_fire = {
    init
      .then(add_ok)
      .then(add_ok)
      .then(rotate)
      .then(add_ok)
      .expect(not(h15_fired))
  }
}
