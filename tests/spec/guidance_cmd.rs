// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen guidance` JSON shape in an empty workspace.

use kaizen::DataSource;

#[test]
fn guidance_json_empty_workspace() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let text = kaizen::shell::guidance::guidance_text(
        Some(tmp.path()),
        7,
        true,
        false,
        DataSource::Local,
    )?;
    assert!(
        text.contains("\"workspace\"") && text.contains("\"rows\""),
        "{}",
        text
    );
    assert!(text.contains("\"sessions_in_window\": 0"));
    Ok(())
}
