use std::path::PathBuf;

pub fn test_socket_path() -> anyhow::Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("aerospace.sock");
    Ok((dir, path))
}
