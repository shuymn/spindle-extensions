use std::process::Command;

use spindle_sketchybar_extension::{cache_file_path, cache_is_current, write_cache};

#[test]
fn invalidate_cache_cli_clears_state_files() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let cache_key = "workspace-indicator.workspaces";
    write_cache(&cache_file_path(dir.path(), cache_key), "snapshot")?;

    let output = Command::new(env!("CARGO_BIN_EXE_spindle-sketchybar"))
        .args(["invalidate-cache", "--state-dir"])
        .arg(dir.path())
        .output()?;

    assert!(
        output.status.success(),
        "invalidate-cache failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!cache_file_path(dir.path(), cache_key).exists());

    Ok(())
}

#[test]
fn invalidate_cache_cli_clears_single_key() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    write_cache(
        &cache_file_path(dir.path(), "workspace-indicator.workspaces"),
        "workspaces",
    )?;
    write_cache(
        &cache_file_path(dir.path(), "workspace-indicator.status.aerospace.layout"),
        "TILE|0xff3fb950",
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_spindle-sketchybar"))
        .args(["invalidate-cache", "--state-dir"])
        .arg(dir.path())
        .args(["--key", "workspace-indicator.workspaces"])
        .output()?;

    assert!(output.status.success());
    assert!(!cache_file_path(dir.path(), "workspace-indicator.workspaces").exists());
    assert!(cache_is_current(
        &cache_file_path(dir.path(), "workspace-indicator.status.aerospace.layout"),
        "TILE|0xff3fb950"
    )?);

    Ok(())
}
