use std::process::Command;

use spindle_sketchybar_extension::{cache_file_path, write_cache};

#[test]
fn default_state_dir_uses_spindle_state_dir_from_child_env() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let cache_key = "workspace-indicator.workspaces";
    let cache_path = cache_file_path(dir.path(), cache_key);
    write_cache(&cache_path, "snapshot")?;

    let output = Command::new(env!("CARGO_BIN_EXE_spindle-sketchybar"))
        .args(["invalidate-cache", "--key", cache_key])
        .env("SPINDLE_STATE_DIR", dir.path())
        .env_remove("SPINDLE_SKETCHYBAR_STATE_DIR")
        .output()?;

    assert!(
        output.status.success(),
        "invalidate-cache failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!cache_path.exists());

    Ok(())
}
