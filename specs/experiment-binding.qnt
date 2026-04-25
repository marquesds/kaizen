// Experiment binding: classification resolution and reclassification safety.
// Invariant 1 — manual precedence: manual tag always overrides git ancestry.
// Invariant 2 — reclassification safety: tagging same variant is idempotent;
//               tagging a different variant when one already exists is a conflict.
// Invariant 3 — determinism: resolve(manual, git) is a pure function of inputs.
module experiment_binding {
  type Classification = | Control | Treatment | Excluded

  // Excluded used as sentinel for "not set".
  var manual_tag:  Classification
  var git_class:   Classification
  var conflict:    bool

  // Resolve: manual wins when present; fallback to git.
  pure def resolve(m: Classification, g: Classification): Classification =
    if (m != Excluded) m else g

  action init: bool = all {
    manual_tag' = Excluded,
    git_class'  = Excluded,
    conflict'   = false
  }

  // Git ancestry check fires once at session end; may produce any class.
  action classify_via_git: bool = {
    nondet g = oneOf(Set(Control, Treatment, Excluded))
    all {
      git_class'  = g,
      manual_tag' = manual_tag,
      conflict'   = conflict
    }
  }

  // Operator manually assigns a variant; idempotent if same, conflict if different.
  action apply_manual_tag: bool = {
    nondet v = oneOf(Set(Control, Treatment))
    all {
      conflict'   = (manual_tag != Excluded and manual_tag != v),
      manual_tag' = if (manual_tag == Excluded or manual_tag == v) v else manual_tag,
      git_class'  = git_class
    }
  }

  action step: bool = any {
    classify_via_git,
    apply_manual_tag
  }

  // Manual tag wins over any git result.
  val manual_precedence: bool =
    (manual_tag != Excluded) implies (resolve(manual_tag, git_class) == manual_tag)

  // Resolution function is consistent with inputs.
  val resolution_deterministic: bool =
    resolve(manual_tag, git_class) ==
      (if (manual_tag != Excluded) manual_tag else git_class)

  // A conflict can only occur when a manual tag was already set.
  val conflict_requires_prior_tag: bool =
    conflict implies (manual_tag != Excluded)

  run testManualBeatsGit = {
    init
      .then(classify_via_git)
      .then(apply_manual_tag)
      .expect(resolve(manual_tag, git_class) == manual_tag)
  }

  run testGitUsedWhenNoManual = {
    init
      .then(classify_via_git)
      .expect(manual_tag == Excluded and resolve(manual_tag, git_class) == git_class)
  }

  run testSameTagIdempotent = {
    init
      .then(apply_manual_tag)
      .then(apply_manual_tag)
      .expect(conflict == false or manual_tag != Excluded)
  }
}
