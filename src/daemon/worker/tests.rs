use super::*;

#[test]
fn store_cache_reuses_open_connection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kaizen.db");
    let mut cache = StoreCache::default();
    let first = cache.open_write(&path).unwrap() as *const Store;
    let second = cache.open_write(&path).unwrap() as *const Store;
    assert_eq!(first, second);
}

#[test]
fn read_cache_rejects_missing_store_without_creation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kaizen.db");
    let mut cache = StoreCache::default();
    assert!(cache.open_read(&path).is_err());
    assert!(!path.exists());
}
