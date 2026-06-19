use std::ffi::OsString;

#[test]
fn init_wires_user_prompt_capture_in_user_config() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let workspace = temp.path().join("workspace");
    std::fs::create_dir(&workspace)?;
    let _home = EnvGuard::set("HOME", temp.path().into());
    let _kaizen = EnvGuard::set("KAIZEN_HOME", temp.path().join("kaizen").into());
    kaizen::shell::init::init_text(Some(&workspace))?;
    let raw = std::fs::read_to_string(temp.path().join(".claude/settings.json"))?;
    let value: serde_json::Value = serde_json::from_str(&raw)?;
    assert!(value.pointer("/hooks/UserPromptSubmit").is_some());
    Ok(())
}

struct EnvGuard(&'static str, Option<OsString>);

impl EnvGuard {
    fn set(key: &'static str, value: OsString) -> Self {
        let old = std::env::var_os(key);
        unsafe { std::env::set_var(key, value) };
        Self(key, old)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.1.take() {
            Some(value) => unsafe { std::env::set_var(self.0, value) },
            None => unsafe { std::env::remove_var(self.0) },
        }
    }
}
