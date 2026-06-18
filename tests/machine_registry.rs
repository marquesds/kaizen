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

#[test]
fn machine_scope_ignores_workspace_containing_kaizen_home() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
    let tmp = tempfile::tempdir()?;
    let workspace = tmp.path().join("repo");
    std::fs::create_dir(&workspace)?;
    kaizen::core::machine_registry::upsert_from_resolve(&workspace)?;
    insert_legacy_home_workspace()?;

    let roots = kaizen::core::workspace::machine_workspaces(Some(&workspace))?;

    assert_eq!(roots, vec![std::fs::canonicalize(workspace)?]);
    Ok(())
}

fn insert_legacy_home_workspace() -> anyhow::Result<()> {
    let home = std::fs::canonicalize(std::env::var("HOME")?)?;
    let db = kaizen::core::machine_registry::db_path().expect("test home");
    let connection = rusqlite::Connection::open(db)?;
    connection.execute(
        "INSERT INTO projects(path, name, first_seen_ms, last_seen_ms) VALUES (?1, 'home', 0, 1)",
        [home.to_string_lossy().as_ref()],
    )?;
    Ok(())
}

fn assert_raw_row(workspace: std::path::PathBuf) -> anyhow::Result<()> {
    let rows = kaizen::core::machine_registry::list_paths_including_missing()?;
    assert_eq!(rows, vec![workspace]);
    Ok(())
}
