//! Migration framework for upgrading older project files to
//! [`CURRENT_FORMAT_VERSION`](super::format::CURRENT_FORMAT_VERSION).
//!
//! On load:
//! 1. Parse the JSON loosely as `serde_json::Value`.
//! 2. Read the `version` field.
//! 3. Walk migrations from `version` up to `CURRENT_FORMAT_VERSION`.
//! 4. Final value parses cleanly as `ProjectFile`.
//!
//! Today there are no migration steps because we ship at v1; the framework
//! is here from day 1 (P0.3) so v2 has somewhere obvious to land.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::format::{ProjectFile, CURRENT_FORMAT_VERSION};

/// Errors that can occur during version migration.
#[derive(Debug, Error)]
pub enum MigrateError {
    /// The file's `version` field could not be read.
    #[error("missing or invalid `version` field")]
    MissingVersion,
    /// The file is from a future version this binary doesn't know about.
    #[error("unsupported future format version {0} (this build supports up to {1})")]
    UnsupportedFutureVersion(u32, u32),
    /// No migration step is registered for this version (data loss risk).
    #[error("no migration registered for version {0}")]
    UnknownVersion(u32),
    /// Final structural decode failed.
    #[error("failed to decode migrated value as ProjectFile: {0}")]
    Decode(#[from] serde_json::Error),
}

/// Probe just the `version` field of a serialized file. Useful for the load
/// path which needs to dispatch on version before fully parsing.
#[derive(Debug, Serialize, Deserialize)]
struct VersionProbe {
    version: u32,
}

/// Read the version off a `serde_json::Value`. Returns
/// `Err(MissingVersion)` if there's no integer `version` field.
pub fn version_of(value: &serde_json::Value) -> Result<u32, MigrateError> {
    value
        .get("version")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or(MigrateError::MissingVersion)
}

/// Migrate a project value from `from` to [`CURRENT_FORMAT_VERSION`] and
/// return the typed `ProjectFile`. If `from` equals the current version, no
/// migration steps run; the value is decoded directly.
pub fn migrate_to_current(
    value: serde_json::Value,
    from: u32,
) -> Result<ProjectFile, MigrateError> {
    if from > CURRENT_FORMAT_VERSION {
        return Err(MigrateError::UnsupportedFutureVersion(from, CURRENT_FORMAT_VERSION));
    }

    let mut current = value;
    let mut version = from;
    while version < CURRENT_FORMAT_VERSION {
        current = match version {
            // No migrations defined yet — every version below
            // CURRENT_FORMAT_VERSION needs a step or we error.
            v => return Err(MigrateError::UnknownVersion(v)),
        };
        // Each step would normally bump `version`. Once a real migration
        // lands here, increment after the conversion.
        #[allow(unreachable_code)]
        {
            version += 1;
        }
    }

    let file: ProjectFile = serde_json::from_value(current)?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn version_probe_reads_field() {
        let v = json!({ "version": 1, "other": "stuff" });
        assert_eq!(version_of(&v).unwrap(), 1);
    }

    #[test]
    fn missing_version_errors() {
        let v = json!({ "no_version_here": true });
        let err = version_of(&v).unwrap_err();
        assert!(matches!(err, MigrateError::MissingVersion));
    }

    #[test]
    fn future_version_rejected() {
        let v = json!({ "version": 9999 });
        let err = migrate_to_current(v, 9999).unwrap_err();
        assert!(matches!(err, MigrateError::UnsupportedFutureVersion(9999, 1)));
    }

    #[test]
    fn current_version_passes_through() {
        // Build a minimal v1 ProjectFile structurally.
        let v = json!({
            "version": 1,
            "meta": { "name": "t", "author": "" },
            "tempo": 120.0,
            "time_signature": [4, 4],
            "tracks": [],
            "clips": [],
            "scenes": []
        });
        let file = migrate_to_current(v, 1).expect("migrate");
        assert_eq!(file.version, 1);
        assert_eq!(file.meta.name, "t");
    }
}
