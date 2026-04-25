// Sequential / always-valid confidence intervals for A/B experiments.
// Core invariant: once the decision is Significant it is sticky — subsequent
// observations cannot downgrade it to Inconclusive or Insufficient.
//
// Model: each observation may or may not carry enough evidence to cross the
// significance threshold. We do not model the exact e-value computation here;
// that lives in `stats::sequential` (Rust). Instead we model the decision FSM
// and the stickiness property.
module experiment_sequential {
  type Decision = | Insufficient | Inconclusive | Significant

  pure val MIN_N: int = 30

  var n_control:        int
  var n_treatment:      int
  var ever_significant: bool   // sticky: true once Significant is reached
  var decision:         Decision

  // Decision rule: sticky once Significant is set.
  pure def decide(nc: int, nt: int, ever_sig: bool): Decision =
    if (ever_sig)
      Significant
    else if (nc < MIN_N or nt < MIN_N)
      Insufficient
    else
      Inconclusive

  action init: bool = all {
    n_control'        = 0,
    n_treatment'      = 0,
    ever_significant' = false,
    decision'         = Insufficient
  }

  // Add one observation to the control arm; evidence may or may not suffice.
  action observe_control: bool = {
    nondet crosses = oneOf(Set(true, false))
    all {
      n_control'        = n_control + 1,
      n_treatment'      = n_treatment,
      ever_significant' = ever_significant or
        (crosses and n_control + 1 >= MIN_N and n_treatment >= MIN_N),
      decision'         = decide(
        n_control + 1,
        n_treatment,
        ever_significant or
          (crosses and n_control + 1 >= MIN_N and n_treatment >= MIN_N)
      )
    }
  }

  // Add one observation to the treatment arm; evidence may or may not suffice.
  action observe_treatment: bool = {
    nondet crosses = oneOf(Set(true, false))
    all {
      n_control'        = n_control,
      n_treatment'      = n_treatment + 1,
      ever_significant' = ever_significant or
        (crosses and n_control >= MIN_N and n_treatment + 1 >= MIN_N),
      decision'         = decide(
        n_control,
        n_treatment + 1,
        ever_significant or
          (crosses and n_control >= MIN_N and n_treatment + 1 >= MIN_N)
      )
    }
  }

  action step: bool = any {
    observe_control,
    observe_treatment
  }

  // Core: once Significant, always Significant.
  val significant_is_sticky: bool =
    ever_significant implies decision == Significant

  // Decision is consistent with current state.
  val decision_consistent: bool =
    decision == decide(n_control, n_treatment, ever_significant)

  // Insufficient only when sample is too small.
  val insufficient_only_small: bool =
    (decision == Insufficient) implies (n_control < MIN_N or n_treatment < MIN_N)

  run testInsufficientAtStart = {
    init
      .expect(decision == Insufficient)
  }

  run testObservationsAccumulate = {
    init
      .then(observe_control)
      .then(observe_treatment)
      .expect(n_control == 1 and n_treatment == 1)
  }
}
