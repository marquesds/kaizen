// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{WebFeature, wf};

pub(super) const FEATURES: &[WebFeature] = &[
    wf(
        "session-detail",
        "Trace",
        "Show span tree",
        "get_session_span_tree",
        &["id"],
        false,
        "tree",
    ),
    wf(
        "session-detail",
        "Feedback",
        "Save feedback",
        "kaizen_annotate_session",
        &["session_id"],
        true,
        "toast",
    ),
    wf(
        "session-detail",
        "Search",
        "Run structured query",
        "kaizen_query",
        &["expr"],
        false,
        "table",
    ),
    wf(
        "session-detail",
        "Trace",
        "Show session",
        "kaizen_session_show",
        &["id"],
        false,
        "detail",
    ),
    wf(
        "session-detail",
        "Live",
        "Open live session view",
        "kaizen_tui",
        &[],
        false,
        "live",
    ),
    wf(
        "session-detail",
        "Search",
        "Search sessions",
        "mcp/search_sessions",
        &["query"],
        false,
        "table",
    ),
];
