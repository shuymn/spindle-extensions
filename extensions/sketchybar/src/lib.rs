//! `SketchyBar` IPC primitives and spindle actions.

#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

mod cache;

use std::{env, path::PathBuf};

pub use cache::{
    EndpointAvailability, cache_file_path, cache_is_current, clear_cache_dir, clear_cache_key,
    default_state_dir, resolve_state_dir, sync_endpoint_lifecycle, validate_cache_key, write_cache,
};
use thiserror::Error;

/// Error returned by the `SketchyBar` extension.
#[derive(Debug, Error)]
pub enum SketchybarError {
    /// `SketchyBar` returned an error response.
    #[error("sketchybar command failed: {0}")]
    CommandFailed(String),

    /// Low-level Mach IPC failed.
    #[error("mach ipc failed: {operation} returned {code}")]
    Mach {
        /// Operation name.
        operation: &'static str,
        /// Native return code.
        code: i32,
    },

    /// This transport is only available on macOS.
    #[error("{transport} transport is only available on macOS")]
    UnsupportedPlatform {
        /// Transport name.
        transport: &'static str,
    },

    /// Cache key validation failed.
    #[error("invalid cache key: {0}")]
    InvalidCacheKey(String),

    /// Filesystem operation failed.
    #[error("io error at {}: {source}", path.display())]
    Io {
        /// Path involved in the failure.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
}

const DEFAULT_SKETCHYBAR_NAME: &str = "sketchybar";

/// `SketchyBar` Mach message payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SketchybarMessage {
    bytes: Vec<u8>,
}

impl SketchybarMessage {
    /// Build the same NUL-separated payload that the `sketchybar` CLI sends.
    #[must_use]
    pub fn from_args<S>(args: &[S]) -> Self
    where
        S: AsRef<str>,
    {
        let total_len = args.iter().map(|arg| arg.as_ref().len() + 1).sum::<usize>() + 1;
        let mut bytes = Vec::with_capacity(total_len);
        for arg in args {
            bytes.extend_from_slice(arg.as_ref().as_bytes());
            bytes.push(0);
        }
        bytes.push(0);
        Self { bytes }
    }

    /// Return the raw wire-format bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Low-level `SketchyBar` client.
pub trait SketchybarClient {
    /// Send a preformatted `SketchyBar` message without spawning the CLI.
    ///
    /// # Errors
    ///
    /// Returns an error when the Mach port cannot be reached or `SketchyBar`
    /// reports a failure.
    fn send_message(&self, message: &SketchybarMessage) -> Result<String, SketchybarError>;

    /// Send command-line style arguments through the `SketchyBar` IPC protocol.
    ///
    /// # Errors
    ///
    /// Returns an error when the Mach port cannot be reached or `SketchyBar`
    /// reports a failure.
    fn send_args<S>(&self, args: &[S]) -> Result<String, SketchybarError>
    where
        S: AsRef<str>,
    {
        self.send_message(&SketchybarMessage::from_args(args))
    }
}

/// Probe whether the `SketchyBar` Mach endpoint is currently registered.
pub trait EndpointProbe {
    /// Return whether the named bar endpoint is reachable.
    ///
    /// # Errors
    ///
    /// Returns an error when the probe cannot be executed on this platform.
    fn availability(&self, bar_name: &str) -> Result<EndpointAvailability, SketchybarError>;
}

/// `SketchyBar` Mach IPC client.
#[derive(Debug, Clone)]
pub struct SketchybarMachClient {
    bar_name: String,
}

impl SketchybarMachClient {
    /// Create a client for a named `SketchyBar` instance.
    #[must_use]
    pub const fn new(bar_name: String) -> Self {
        Self { bar_name }
    }

    /// Create a client for the current `SketchyBar` instance.
    #[must_use]
    pub fn from_env() -> Self {
        let bar_name =
            env::var("BAR_NAME").unwrap_or_else(|_err| String::from(DEFAULT_SKETCHYBAR_NAME));
        Self::new(bar_name)
    }

    /// Return the configured bar name.
    #[must_use]
    pub fn bar_name(&self) -> &str {
        &self.bar_name
    }
}

impl Default for SketchybarMachClient {
    fn default() -> Self {
        Self::from_env()
    }
}

impl SketchybarClient for SketchybarMachClient {
    fn send_message(&self, message: &SketchybarMessage) -> Result<String, SketchybarError> {
        sketchybar_ipc::send(&self.bar_name, message.as_bytes())
    }
}

/// Mach-backed endpoint probe.
#[derive(Debug, Clone, Default)]
pub struct MachEndpointProbe;

impl EndpointProbe for MachEndpointProbe {
    fn availability(&self, bar_name: &str) -> Result<EndpointAvailability, SketchybarError> {
        match sketchybar_ipc::probe(bar_name) {
            Ok(()) => Ok(EndpointAvailability::Available),
            Err(SketchybarError::Mach { .. }) => Ok(EndpointAvailability::Unavailable),
            Err(error) => Err(error),
        }
    }
}

/// Send raw `SketchyBar` command arguments through Mach IPC.
///
/// # Errors
///
/// Returns an error when the Mach port cannot be reached or `SketchyBar` reports
/// a failure.
pub fn send_args(args: &[String]) -> Result<String, SketchybarError> {
    SketchybarMachClient::from_env().send_args(args)
}

/// Inputs for a cached or uncached `SketchyBar` send.
pub struct CachedMessageRequest<'a> {
    /// Optional override for the cache directory.
    pub state_dir: Option<PathBuf>,
    /// `SketchyBar` command arguments.
    pub args: &'a [String],
    /// Optional write-cache key and value pair.
    pub cache: Option<&'a (String, String)>,
    /// Target bar name used for endpoint lifecycle tracking.
    pub bar_name: &'a str,
}

/// Send a message with optional write-cache semantics.
///
/// # Errors
///
/// Returns an error when cache files cannot be read or written, or when Mach IPC
/// fails.
pub fn send_cached_message(
    request: &CachedMessageRequest<'_>,
    client: &impl SketchybarClient,
    probe: &impl EndpointProbe,
) -> Result<(), SketchybarError> {
    let Some((cache_key, cache_value)) = request.cache else {
        client.send_args(request.args)?;
        return Ok(());
    };

    let state_dir = resolve_state_dir(request.state_dir.clone());
    let availability = probe.availability(request.bar_name)?;
    sync_endpoint_lifecycle(&state_dir, request.bar_name, availability)?;

    validate_cache_key(cache_key)?;
    let cache_path = cache_file_path(&state_dir, cache_key);
    if cache_is_current(&cache_path, cache_value)? {
        return Ok(());
    }

    if let Err(error) = client.send_args(request.args) {
        let _ = clear_cache_key(&state_dir, cache_key);
        return Err(error);
    }
    write_cache(&cache_path, cache_value)?;
    Ok(())
}

/// Invalidate write-cache entries under the resolved state directory.
///
/// # Errors
///
/// Returns an error when cache files cannot be removed.
pub fn invalidate_cache(
    state_dir: Option<PathBuf>,
    cache_key: Option<&str>,
) -> Result<(), SketchybarError> {
    let state_dir = resolve_state_dir(state_dir);
    match cache_key {
        Some(key) => {
            validate_cache_key(key)?;
            clear_cache_key(&state_dir, key)
        }
        None => clear_cache_dir(&state_dir),
    }
}

#[cfg(target_os = "macos")]
// SketchyBar updates are latency-sensitive; use one isolated FFI bridge to
// avoid spawning the `sketchybar` CLI for every message.
#[allow(unsafe_code)]
mod sketchybar_ipc {
    use std::ffi::{CString, c_char};

    use crate::SketchybarError;

    unsafe extern "C" {
        fn spindle_sketchybar_probe(bar_name: *const c_char) -> i32;
        fn spindle_sketchybar_send(
            bar_name: *const c_char,
            message: *const u8,
            message_len: u32,
        ) -> i32;
    }

    pub fn probe(bar_name: &str) -> Result<(), SketchybarError> {
        let bar_name = CString::new(bar_name).map_err(|_err| {
            SketchybarError::CommandFailed(String::from("bar name contains NUL"))
        })?;
        let code = unsafe { spindle_sketchybar_probe(bar_name.as_ptr()) };
        if code == 0 {
            Ok(())
        } else {
            Err(SketchybarError::Mach {
                operation: "spindle_sketchybar_probe",
                code,
            })
        }
    }

    pub fn send(bar_name: &str, bytes: &[u8]) -> Result<String, SketchybarError> {
        let bar_name = CString::new(bar_name).map_err(|_err| {
            SketchybarError::CommandFailed(String::from("bar name contains NUL"))
        })?;
        let message_len = u32::try_from(bytes.len()).map_err(|_err| SketchybarError::Mach {
            operation: "message_len",
            code: -1,
        })?;

        let code =
            unsafe { spindle_sketchybar_send(bar_name.as_ptr(), bytes.as_ptr(), message_len) };
        if code == 0 {
            Ok(String::new())
        } else {
            Err(SketchybarError::Mach {
                operation: "spindle_sketchybar_send",
                code,
            })
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod sketchybar_ipc {
    use crate::SketchybarError;

    pub(super) fn probe(_bar_name: &str) -> Result<(), SketchybarError> {
        Err(SketchybarError::UnsupportedPlatform {
            transport: "sketchybar mach",
        })
    }

    pub(super) fn send(_bar_name: &str, _bytes: &[u8]) -> Result<String, SketchybarError> {
        Err(SketchybarError::UnsupportedPlatform {
            transport: "sketchybar mach",
        })
    }
}

#[cfg(test)]
mod send_tests {
    use std::{
        cell::RefCell,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    fn temp_state_dir() -> Result<tempfile::TempDir, SketchybarError> {
        tempfile::tempdir().map_err(|source| SketchybarError::Io {
            path: std::env::temp_dir(),
            source,
        })
    }

    struct RecordingClient {
        sends: AtomicUsize,
    }

    impl SketchybarClient for RecordingClient {
        fn send_message(&self, _message: &SketchybarMessage) -> Result<String, SketchybarError> {
            self.sends.fetch_add(1, Ordering::SeqCst);
            Ok(String::new())
        }
    }

    struct FailingClient;

    impl SketchybarClient for FailingClient {
        fn send_message(&self, _message: &SketchybarMessage) -> Result<String, SketchybarError> {
            Err(SketchybarError::Mach {
                operation: "test",
                code: -1,
            })
        }
    }

    struct StubProbe {
        availability: RefCell<EndpointAvailability>,
    }

    impl EndpointProbe for StubProbe {
        fn availability(&self, _bar_name: &str) -> Result<EndpointAvailability, SketchybarError> {
            Ok(*self.availability.borrow())
        }
    }

    #[test]
    fn uncached_message_sends_without_writing_cache_files() -> Result<(), SketchybarError> {
        let dir = temp_state_dir()?;
        let client = RecordingClient {
            sends: AtomicUsize::new(0),
        };
        let probe = StubProbe {
            availability: RefCell::new(EndpointAvailability::Available),
        };
        let args = vec![String::from("--set"), String::from("clock.date")];

        send_cached_message(
            &CachedMessageRequest {
                state_dir: Some(dir.path().to_path_buf()),
                args: &args,
                cache: None,
                bar_name: "sketchybar",
            },
            &client,
            &probe,
        )?;

        assert_eq!(client.sends.load(Ordering::SeqCst), 1);
        let entries = std::fs::read_dir(dir.path()).map_err(|source| SketchybarError::Io {
            path: dir.path().to_path_buf(),
            source,
        })?;
        assert_eq!(entries.count(), 0);
        Ok(())
    }

    #[test]
    fn cache_hit_skips_send() -> Result<(), SketchybarError> {
        let dir = temp_state_dir()?;
        let cache_key = String::from("test.workspaces");
        let cache_value = String::from("2\n 1  2 \n");
        write_cache(&cache_file_path(dir.path(), &cache_key), &cache_value)?;
        sync_endpoint_lifecycle(dir.path(), "sketchybar", EndpointAvailability::Available)?;

        let client = RecordingClient {
            sends: AtomicUsize::new(0),
        };
        let probe = StubProbe {
            availability: RefCell::new(EndpointAvailability::Available),
        };
        let args = vec![String::from("--set"), String::from("ws.1")];

        send_cached_message(
            &CachedMessageRequest {
                state_dir: Some(dir.path().to_path_buf()),
                args: &args,
                cache: Some(&(cache_key, cache_value)),
                bar_name: "sketchybar",
            },
            &client,
            &probe,
        )?;

        assert_eq!(client.sends.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[test]
    fn cache_miss_sends_and_writes_cache() -> Result<(), SketchybarError> {
        let dir = temp_state_dir()?;
        let cache_key = String::from("test.workspaces");
        let cache_value = String::from("2\n 1  2 \n");
        sync_endpoint_lifecycle(dir.path(), "sketchybar", EndpointAvailability::Available)?;

        let client = RecordingClient {
            sends: AtomicUsize::new(0),
        };
        let probe = StubProbe {
            availability: RefCell::new(EndpointAvailability::Available),
        };
        let args = vec![String::from("--set"), String::from("ws.1")];

        send_cached_message(
            &CachedMessageRequest {
                state_dir: Some(dir.path().to_path_buf()),
                args: &args,
                cache: Some(&(cache_key.clone(), cache_value.clone())),
                bar_name: "sketchybar",
            },
            &client,
            &probe,
        )?;

        assert_eq!(client.sends.load(Ordering::SeqCst), 1);
        assert!(cache_is_current(
            &cache_file_path(dir.path(), &cache_key),
            &cache_value
        )?);
        Ok(())
    }

    #[test]
    fn endpoint_restart_allows_resend_with_same_cache_value() -> Result<(), SketchybarError> {
        let dir = temp_state_dir()?;
        let cache_key = String::from("test.status.layout");
        let cache_value = String::from("TILE|0xff3fb950");
        write_cache(&cache_file_path(dir.path(), &cache_key), &cache_value)?;
        sync_endpoint_lifecycle(dir.path(), "sketchybar", EndpointAvailability::Unavailable)?;

        let client = RecordingClient {
            sends: AtomicUsize::new(0),
        };
        let probe = StubProbe {
            availability: RefCell::new(EndpointAvailability::Available),
        };
        let args = vec![
            String::from("--set"),
            String::from("aerospace.layout"),
            String::from("label=TILE"),
            String::from("label.color=0xff3fb950"),
        ];

        send_cached_message(
            &CachedMessageRequest {
                state_dir: Some(dir.path().to_path_buf()),
                args: &args,
                cache: Some(&(cache_key, cache_value)),
                bar_name: "sketchybar",
            },
            &client,
            &probe,
        )?;

        assert_eq!(client.sends.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[test]
    fn mach_send_failure_clears_affected_cache_key() -> Result<(), SketchybarError> {
        let dir = temp_state_dir()?;
        let cache_key = String::from("test.workspaces");
        let stale_value = String::from("stale-snapshot");
        let cache_value = String::from("new-snapshot");
        write_cache(&cache_file_path(dir.path(), &cache_key), &stale_value)?;
        sync_endpoint_lifecycle(dir.path(), "sketchybar", EndpointAvailability::Available)?;

        let client = FailingClient;
        let probe = StubProbe {
            availability: RefCell::new(EndpointAvailability::Available),
        };
        let args = vec![String::from("--set"), String::from("ws.1")];

        let result = send_cached_message(
            &CachedMessageRequest {
                state_dir: Some(dir.path().to_path_buf()),
                args: &args,
                cache: Some(&(cache_key.clone(), cache_value)),
                bar_name: "sketchybar",
            },
            &client,
            &probe,
        );

        assert!(result.is_err());
        assert!(!cache_file_path(dir.path(), &cache_key).exists());
        Ok(())
    }
}
