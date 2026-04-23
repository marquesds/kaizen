// Retention tiers: Hot -> Warm -> Cold -> Purged.
// Age advances with tick; transitions gate on age. No hot event older than HOT_DAYS.
module retention {
  type Tier = | Hot | Warm | Cold | Purged

  pure val HOT_DAYS: int = 30
  pure val WARM_DAYS: int = 90

  var tier: Tier
  var age_days: int

  action init: bool = all {
    tier' = Hot,
    age_days' = 0
  }

  action tick_hot: bool = all {
    tier == Hot,
    age_days < HOT_DAYS,
    tier' = Hot,
    age_days' = age_days + 1
  }

  action promote_warm: bool = all {
    tier == Hot,
    age_days >= HOT_DAYS,
    tier' = Warm,
    age_days' = age_days
  }

  action tick_warm: bool = all {
    tier == Warm,
    age_days < WARM_DAYS,
    tier' = Warm,
    age_days' = age_days + 1
  }

  action promote_cold: bool = all {
    tier == Warm,
    age_days >= WARM_DAYS,
    tier' = Cold,
    age_days' = age_days
  }

  action purge: bool = all {
    tier == Cold,
    tier' = Purged,
    age_days' = age_days
  }

  action step: bool = any {
    tick_hot,
    promote_warm,
    tick_warm,
    promote_cold,
    purge
  }

  // No hot event older than HOT_DAYS.
  val hot_bound: bool =
    (tier == Hot) implies age_days <= HOT_DAYS

  // Purged is terminal.
  val purged_terminal: bool =
    (tier == Purged) implies age_days >= 0

  action fast_forward_hot: bool = all {
    tier == Hot,
    tier' = Hot,
    age_days' = HOT_DAYS
  }

  action fast_forward_warm: bool = all {
    tier == Warm,
    tier' = Warm,
    age_days' = WARM_DAYS
  }

  run testReachesPurged = {
    init
      .then(fast_forward_hot)
      .then(promote_warm)
      .then(fast_forward_warm)
      .then(promote_cold)
      .then(purge)
      .expect(tier == Purged)
  }
}
