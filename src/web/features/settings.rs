// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{WebFeature, wf};

pub(super) const FEATURES: &[WebFeature] = &[
    wf(
        "settings",
        "Runtime",
        "Show capabilities",
        "kaizen_capabilities",
        &[],
        false,
        "markdown",
    ),
    wf(
        "settings",
        "Capture",
        "Ingest hook payload",
        "kaizen_ingest_hook",
        &["source", "payload"],
        true,
        "toast",
    ),
    wf(
        "settings",
        "Runtime",
        "Initialize workspace",
        "kaizen_init",
        &[],
        true,
        "markdown",
    ),
    wf(
        "settings",
        "Sync",
        "Run sync once",
        "kaizen_sync_run",
        &[],
        true,
        "toast",
    ),
    wf(
        "settings",
        "Sync",
        "Check sync status",
        "kaizen_sync_status",
        &[],
        false,
        "detail",
    ),
];
