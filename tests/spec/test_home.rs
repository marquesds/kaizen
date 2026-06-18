use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard, OnceLock};

pub(crate) struct TestHome {
    _lock: MutexGuard<'static, ()>,
    _temp: tempfile::TempDir,
    home: Option<OsString>,
    kaizen_home: Option<OsString>,
}

impl TestHome {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let temp = tempfile::tempdir()?;
        let home = std::env::var_os("HOME");
        let kaizen_home = std::env::var_os("KAIZEN_HOME");
        set_homes(temp.path());
        Ok(Self {
            _lock: lock,
            _temp: temp,
            home,
            kaizen_home,
        })
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        restore("HOME", self.home.take());
        restore("KAIZEN_HOME", self.kaizen_home.take());
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn set_homes(home: &std::path::Path) {
    unsafe {
        std::env::set_var("HOME", home);
        std::env::set_var("KAIZEN_HOME", home.join(".kaizen"));
    }
}

fn restore(name: &str, value: Option<OsString>) {
    unsafe {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}
