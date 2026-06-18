// SPDX-License-Identifier: AGPL-3.0-or-later

#[path = "spec/test_home.rs"]
mod test_home;

#[test]
fn upsert_and_list() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
    let tmp = tempfile::tempdir()?;
    let ws = tmp.path().join("repo");
    std::fs::create_dir(&ws)?;
    kaizen::core::machine_registry::upsert_from_resolve(&ws)?;
    let paths = kaizen::core::machine_registry::list_paths()?;
    assert_eq!(paths, vec![std::fs::canonicalize(ws)?]);
    assert!(kaizen::core::machine_registry::is_registered(&paths[0]));
    Ok(())
}

#[test]
fn default_list_omits_missing_workspace_without_deleting_row() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
    let tmp = tempfile::tempdir()?;
    let ws = tmp.path().join("gone");
    std::fs::create_dir(&ws)?;
    kaizen::core::machine_registry::upsert_from_resolve(&ws)?;
    let ws = std::fs::canonicalize(ws)?;
    std::fs::remove_dir(&ws)?;
    assert!(kaizen::core::machine_registry::list_paths()?.is_empty());
    assert_raw_row(ws)?;
    Ok(())
}

fn assert_raw_row(workspace: std::path::PathBuf) -> anyhow::Result<()> {
    let rows = kaizen::core::machine_registry::list_paths_including_missing()?;
    assert_eq!(rows, vec![workspace]);
    Ok(())
}
