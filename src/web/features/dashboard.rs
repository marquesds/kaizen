// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{WebFeature, wf};

pub(super) const FEATURES: &[WebFeature] = &[
    wf(
        "dashboard",
        "Insights",
        "Refresh insights",
        "kaizen_insights",
        &[],
        false,
        "cards",
    ),
    wf(
        "dashboard",
        "Metrics",
        "Refresh metrics",
        "kaizen_metrics",
        &[],
        false,
        "metrics",
    ),
    wf(
        "dashboard",
        "Sessions",
        "Refresh sessions",
        "kaizen_sessions_list",
        &[],
        false,
        "table",
    ),
    wf(
        "dashboard",
        "Summary",
        "Refresh summary",
        "kaizen_summary",
        &[],
        false,
        "summary",
    ),
];
