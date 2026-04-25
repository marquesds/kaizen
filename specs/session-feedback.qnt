// H17 trigger invariants for session-level human feedback.
// FeedbackScore: integer 1..5. "Low" mean threshold: mean <= 2.5 (i.e. sum*2 <= n_scored*5).
// H17 fires when: n_bad >= 2 OR (n_scored >= 5 AND mean <= 2.5).
// h17_fired is sticky within a window.
module session_feedback {
  var n_bad: int
  var n_scored: int
  var sum_scores: int
  var h17_fired: bool

  pure def trigger(n_b: int, n_s: int, s: int): bool =
    n_b >= 2
    or (n_s >= 5 and s * 2 <= n_s * 5)

  action init: bool = all {
    n_bad' = 0,
    n_scored' = 0,
    sum_scores' = 0,
    h17_fired' = false
  }

  // Label bad/regression, no score
  action add_bad: bool = all {
    n_bad' = n_bad + 1,
    n_scored' = n_scored,
    sum_scores' = sum_scores,
    h17_fired' = h17_fired or trigger(n_bad + 1, n_scored, sum_scores)
  }

  // Label good/interesting, no score
  action add_good: bool = all {
    n_bad' = n_bad,
    n_scored' = n_scored,
    sum_scores' = sum_scores,
    h17_fired' = h17_fired
  }

  // Score 2 — low
  action add_scored_low: bool = all {
    n_bad' = n_bad,
    n_scored' = n_scored + 1,
    sum_scores' = sum_scores + 2,
    h17_fired' = h17_fired or trigger(n_bad, n_scored + 1, sum_scores + 2)
  }

  // Score 4 — high
  action add_scored_high: bool = all {
    n_bad' = n_bad,
    n_scored' = n_scored + 1,
    sum_scores' = sum_scores + 4,
    h17_fired' = h17_fired or trigger(n_bad, n_scored + 1, sum_scores + 4)
  }

  action step: bool = any {
    add_bad,
    add_good,
    add_scored_low,
    add_scored_high
  }

  val counts_non_negative: bool =
    n_bad >= 0 and n_scored >= 0 and sum_scores >= 0
  val scored_le_total: bool = n_scored >= 0
  val no_fire_empty: bool = (n_bad == 0 and n_scored == 0) implies not(h17_fired)
  val sum_bounded: bool = sum_scores <= n_scored * 5

  run two_bad_fires = {
    init
      .then(add_bad)
      .then(add_bad)
      .expect(h17_fired)
  }

  run one_bad_no_fire = {
    init
      .then(add_bad)
      .expect(not(h17_fired))
  }

  run five_low_scores_fires = {
    init
      .then(add_scored_low)
      .then(add_scored_low)
      .then(add_scored_low)
      .then(add_scored_low)
      .then(add_scored_low)
      .expect(h17_fired)
  }

  run four_low_scores_no_fire = {
    init
      .then(add_scored_low)
      .then(add_scored_low)
      .then(add_scored_low)
      .then(add_scored_low)
      .expect(not(h17_fired))
  }

  run five_high_scores_no_fire = {
    init
      .then(add_scored_high)
      .then(add_scored_high)
      .then(add_scored_high)
      .then(add_scored_high)
      .then(add_scored_high)
      .expect(not(h17_fired))
  }

  run h17_sticky = {
    init
      .then(add_bad)
      .then(add_bad)
      .then(add_good)
      .expect(h17_fired)
  }

  run bad_and_high_scores_still_fires = {
    init
      .then(add_bad)
      .then(add_bad)
      .then(add_scored_high)
      .expect(h17_fired)
  }

  run mixed_mean_at_threshold_fires = {
    // sum=12 n=5 mean=2.4 <= 2.5 → fires
    init
      .then(add_scored_low)  // 2
      .then(add_scored_low)  // 4
      .then(add_scored_low)  // 6
      .then(add_scored_low)  // 8
      .then(add_scored_high) // 12
      .expect(h17_fired)
  }
}
