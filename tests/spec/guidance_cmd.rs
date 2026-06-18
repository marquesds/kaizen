// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen guidance` JSON shape in an empty workspace.

use kaizen::DataSource;

mod test_home;

#[test]
fn guidance_json_empty_workspace() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
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

#[test]
fn guidance_score_json_empty_workspace() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
    let tmp = tempfile::tempdir()?;
    let text = kaizen::shell::guidance_science::score_text(Some(tmp.path()), 30, 30, true)?;
    assert!(text.contains("\"rows\": []"), "{text}");
    Ok(())
}

#[test]
fn guidance_propose_never_changes_rule() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
    let tmp = tempfile::tempdir()?;
    let rules = tmp.path().join(".cursor/rules");
    std::fs::create_dir_all(&rules)?;
    std::fs::write(rules.join("dead.mdc"), "rule text")?;
    let text = kaizen::shell::guidance_science::propose_text(
        Some(tmp.path()),
        "rule:dead",
        3,
        false,
        true,
    )?;
    assert!(text.contains("\"status\": \"proposed\""), "{text}");
    assert_eq!(
        std::fs::read_to_string(rules.join("dead.mdc"))?,
        "rule text"
    );
    Ok(())
}
