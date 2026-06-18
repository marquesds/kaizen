pub(super) const MIGRATIONS: &[&str] = &["CREATE TABLE IF NOT EXISTS projects (
        path TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        first_seen_ms INTEGER NOT NULL,
        last_seen_ms INTEGER NOT NULL,
        last_init_ms INTEGER,
        init_count INTEGER NOT NULL DEFAULT 0,
        git_remote_origin TEXT,
        kaizen_version_at_init TEXT,
        meta TEXT
    )"];

pub(super) const IMPORT_LEGACY: &str = "INSERT INTO projects
    (path, name, first_seen_ms, last_seen_ms, last_init_ms, init_count,
     git_remote_origin, kaizen_version_at_init, meta)
    VALUES (?1, ?2, ?3, ?4, NULL, 0, NULL, NULL, NULL)
    ON CONFLICT(path) DO UPDATE SET
      last_seen_ms = MAX(projects.last_seen_ms, excluded.last_seen_ms),
      name = excluded.name";

pub(super) const UPSERT_SEEN: &str = "INSERT INTO projects
    (path, name, first_seen_ms, last_seen_ms, last_init_ms, init_count,
     git_remote_origin, kaizen_version_at_init, meta)
    VALUES (?1, ?2, ?3, ?4, NULL, 0, NULL, NULL, NULL)
    ON CONFLICT(path) DO UPDATE SET
      name = excluded.name,
      last_seen_ms = MAX(projects.last_seen_ms, excluded.last_seen_ms),
      first_seen_ms = projects.first_seen_ms";

pub(super) const RECORD_INIT: &str = "INSERT INTO projects
    (path, name, first_seen_ms, last_seen_ms, last_init_ms, init_count,
     git_remote_origin, kaizen_version_at_init, meta)
    VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, NULL)
    ON CONFLICT(path) DO UPDATE SET
      name = excluded.name,
      last_seen_ms = MAX(projects.last_seen_ms, excluded.last_seen_ms),
      last_init_ms = excluded.last_init_ms,
      init_count = projects.init_count + 1,
      git_remote_origin = COALESCE(excluded.git_remote_origin, projects.git_remote_origin),
      kaizen_version_at_init = excluded.kaizen_version_at_init,
      first_seen_ms = projects.first_seen_ms";

pub(super) const LIST_PATHS: &str = "SELECT path FROM projects ORDER BY last_seen_ms DESC";
pub(super) const IS_REGISTERED: &str = "SELECT 1 FROM projects WHERE path = ?1";
pub(super) const PROJECT_COUNT: &str = "SELECT COUNT(*) FROM projects";
