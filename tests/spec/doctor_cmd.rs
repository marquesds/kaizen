// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen doctor` returns version line and zero exit in a temp workspace with valid store.

#[test]
fn doctor_runs_in_temp_workspace() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let (code, text) = kaizen::shell::doctor::doctor_text(Some(tmp.path()))?;
    assert_eq!(code, 0, "expected success, got: {text}");
    assert!(
        text.contains("kaizen") && text.contains("doctor"),
        "{}",
        text
    );
    assert!(
        text.contains("store: OK") || text.contains("sessions in store"),
        "{}",
        text
    );
    // Ensure hook checks don't crash without cursor/claude files
    assert!(text.contains("hooks:"), "{}", text);
    assert!(
        text.contains("machine registry:"),
        "expected machine registry line: {}",
        text
    );
    Ok(())
}
