//! Write-cache and `SketchyBar` endpoint lifecycle tracking.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use crate::SketchybarError;

const ENDPOINT_MARKER: &str = "sketchybar.endpoint";
const STATE_EXTENSION: &str = "state";
const STATE_SUFFIX: &str = ".state";
const STATUS_AVAILABLE: &str = "available";
const STATUS_UNAVAILABLE: &str = "unavailable";

/// Whether the `SketchyBar` Mach endpoint is currently reachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointAvailability {
    /// `bootstrap_look_up` succeeded for the bar service.
    Available,
    /// The bar service is not registered.
    Unavailable,
}

/// Parsed endpoint marker persisted under the cache directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointMarker {
    bar_name: String,
    availability: EndpointAvailability,
}

impl EndpointMarker {
    fn as_line(&self) -> String {
        let status = match self.availability {
            EndpointAvailability::Available => STATUS_AVAILABLE,
            EndpointAvailability::Unavailable => STATUS_UNAVAILABLE,
        };
        format!("{}|{status}", self.bar_name)
    }

    fn parse(contents: &str) -> Option<Self> {
        let (bar_name, status) = contents.trim().split_once('|')?;
        let availability = match status {
            STATUS_AVAILABLE => EndpointAvailability::Available,
            STATUS_UNAVAILABLE => EndpointAvailability::Unavailable,
            _ => return None,
        };
        Some(Self {
            bar_name: String::from(bar_name),
            availability,
        })
    }
}

/// Resolve the directory used for write-cache files.
#[must_use]
pub fn resolve_state_dir(explicit: Option<PathBuf>) -> PathBuf {
    explicit.unwrap_or_else(default_state_dir)
}

/// Default cache directory from environment variables.
#[must_use]
pub fn default_state_dir() -> PathBuf {
    env_path("SPINDLE_SKETCHYBAR_STATE_DIR")
        .or_else(|| env_path("SPINDLE_STATE_DIR"))
        .unwrap_or_else(|| tmp_dir().join("sketchybar-cache"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn tmp_dir() -> PathBuf {
    env::var_os("TMPDIR")
        .filter(|value| !value.is_empty())
        .map_or_else(|| Path::new("/tmp").to_path_buf(), PathBuf::from)
}

/// Validate a cache key before using it as a filename stem.
///
/// # Errors
///
/// Returns an error when the key is empty or contains invalid characters.
pub fn validate_cache_key(cache_key: &str) -> Result<(), SketchybarError> {
    if cache_key.trim().is_empty() {
        return Err(SketchybarError::InvalidCacheKey(String::from(
            "cache key must not be empty",
        )));
    }
    if cache_key
        .chars()
        .any(|character| character.is_control() || character == '/')
    {
        return Err(SketchybarError::InvalidCacheKey(format!(
            "cache key contains invalid characters: {cache_key:?}"
        )));
    }
    Ok(())
}

/// Return whether the cache file already contains `value`.
///
/// # Errors
///
/// Returns an error when the cache file cannot be read.
pub fn cache_is_current(path: &Path, value: &str) -> Result<bool, SketchybarError> {
    match fs::read_to_string(path) {
        Ok(previous) => Ok(previous == value),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(SketchybarError::Io {
            path: path.to_path_buf(),
            source: error,
        }),
    }
}

/// Atomically write a cache value.
///
/// # Errors
///
/// Returns an error when the cache directory or file cannot be written.
pub fn write_cache(path: &Path, value: &str) -> Result<(), SketchybarError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SketchybarError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp_path, value).map_err(|source| SketchybarError::Io {
        path: tmp_path.clone(),
        source,
    })?;
    fs::rename(&tmp_path, path).map_err(|source| SketchybarError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// Delete one cache entry.
///
/// # Errors
///
/// Returns an error when the cache file cannot be removed.
pub fn clear_cache_key(state_dir: &Path, cache_key: &str) -> Result<(), SketchybarError> {
    let path = cache_file_path(state_dir, cache_key);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SketchybarError::Io { path, source }),
    }
}

/// Delete all `*.state` files and the endpoint marker under `state_dir`.
///
/// # Errors
///
/// Returns an error when a cache file cannot be removed.
pub fn clear_cache_dir(state_dir: &Path) -> Result<(), SketchybarError> {
    let entries = match fs::read_dir(state_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(SketchybarError::Io {
                path: state_dir.to_path_buf(),
                source,
            });
        }
    };

    for entry in entries {
        let entry = entry.map_err(|source| SketchybarError::Io {
            path: state_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if should_clear_cache_entry(&path) {
            fs::remove_file(&path).map_err(|source| SketchybarError::Io {
                path: path.clone(),
                source,
            })?;
        }
    }
    Ok(())
}

fn should_clear_cache_entry(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == STATE_EXTENSION)
        || path.file_name().is_some_and(|name| name == ENDPOINT_MARKER)
}

fn endpoint_marker_path(state_dir: &Path) -> PathBuf {
    state_dir.join(ENDPOINT_MARKER)
}

fn read_endpoint_marker(state_dir: &Path) -> Result<Option<EndpointMarker>, SketchybarError> {
    let path = endpoint_marker_path(state_dir);
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(EndpointMarker::parse(&contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SketchybarError::Io { path, source }),
    }
}

fn write_endpoint_marker(state_dir: &Path, marker: &EndpointMarker) -> Result<(), SketchybarError> {
    write_cache(&endpoint_marker_path(state_dir), &marker.as_line())
}

fn should_invalidate_cache(previous: Option<&EndpointMarker>, current: &EndpointMarker) -> bool {
    let Some(previous) = previous else {
        return false;
    };

    if previous.bar_name != current.bar_name {
        return true;
    }

    matches!(
        (previous.availability, current.availability),
        (
            EndpointAvailability::Unavailable,
            EndpointAvailability::Available
        )
    )
}

/// Update endpoint lifecycle state and invalidate stale cache entries when needed.
///
/// # Errors
///
/// Returns an error when cache files cannot be read or written.
pub fn sync_endpoint_lifecycle(
    state_dir: &Path,
    bar_name: &str,
    availability: EndpointAvailability,
) -> Result<(), SketchybarError> {
    let previous = read_endpoint_marker(state_dir)?;
    if previous
        .as_ref()
        .is_some_and(|marker| marker.bar_name == bar_name && marker.availability == availability)
    {
        return Ok(());
    }
    let current = EndpointMarker {
        bar_name: String::from(bar_name),
        availability,
    };
    if should_invalidate_cache(previous.as_ref(), &current) {
        clear_cache_dir(state_dir)?;
    }
    write_endpoint_marker(state_dir, &current)
}

/// Build the cache file path for a cache key.
#[must_use]
pub fn cache_file_path(state_dir: &Path, cache_key: &str) -> PathBuf {
    state_dir.join(format!("{cache_key}{STATE_SUFFIX}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_state_dir() -> Result<tempfile::TempDir, SketchybarError> {
        tempfile::tempdir().map_err(|source| SketchybarError::Io {
            path: std::env::temp_dir(),
            source,
        })
    }

    #[test]
    fn resolve_state_dir_prefers_explicit_path() -> Result<(), SketchybarError> {
        let dir = temp_state_dir()?;
        let path = dir.path().to_path_buf();
        assert_eq!(resolve_state_dir(Some(path.clone())), path);
        Ok(())
    }

    #[test]
    fn endpoint_transition_clears_state_files() -> Result<(), SketchybarError> {
        let dir = temp_state_dir()?;
        write_cache(&cache_file_path(dir.path(), "test.workspaces"), "snapshot")?;
        sync_endpoint_lifecycle(dir.path(), "sketchybar", EndpointAvailability::Unavailable)?;

        sync_endpoint_lifecycle(dir.path(), "sketchybar", EndpointAvailability::Available)?;

        assert!(!cache_file_path(dir.path(), "test.workspaces").exists());
        let marker = read_endpoint_marker(dir.path())?;
        assert_eq!(
            marker,
            Some(EndpointMarker {
                bar_name: String::from("sketchybar"),
                availability: EndpointAvailability::Available,
            })
        );

        Ok(())
    }

    #[test]
    fn bar_name_change_clears_state_files() -> Result<(), SketchybarError> {
        let dir = temp_state_dir()?;
        write_cache(
            &cache_file_path(dir.path(), "test.status.layout"),
            "TILE|0xff3fb950",
        )?;
        sync_endpoint_lifecycle(dir.path(), "sketchybar", EndpointAvailability::Available)?;

        sync_endpoint_lifecycle(dir.path(), "other-bar", EndpointAvailability::Available)?;

        assert!(!cache_file_path(dir.path(), "test.status.layout").exists());

        Ok(())
    }
}
