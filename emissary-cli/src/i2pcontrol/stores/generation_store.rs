// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

//! Versioned generation store for restart-safe persistence.
//!
//! # Design
//!
//! - Each committed state is a complete versioned envelope.
//! - Files use unique, monotonically ordered generation names.
//! - Publication writes a new file rather than overwriting the active file.
//! - Content is serialized deterministically.
//! - Publication uses a same-filesystem rename.
//! - Loaders enumerate bounded candidate generations newest-first.
//! - Only a fully parsed, validated generation becomes active.
//! - A corrupt newest generation falls back to the previous valid generation.
//! - Retention keeps a bounded number of known-good prior generations.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::i2pcontrol::domain::revision::StateRevision;

/// Schema identifier for persistence envelopes.
pub const SCHEMA_IDENTIFIER: &str = "emissary-i2pcontrol";

/// Current schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// Maximum number of generation files to scan during load.
const MAX_GENERATION_SCAN: usize = 100;

/// Maximum number of prior good generations to retain.
const MAX_RETENTION: usize = 5;

/// A versioned persistence envelope.
///
/// Each store wraps its payload in this envelope for versioned, deterministic
/// serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Schema identifier.
    pub schema: String,

    /// Schema version.
    pub version: u32,

    /// The revision of this generation.
    pub revision: StateRevision,

    /// The payload data.
    pub payload: T,
}

impl<T> Envelope<T> {
    /// Create a new envelope.
    pub fn new(revision: StateRevision, payload: T) -> Self {
        Self {
            schema: SCHEMA_IDENTIFIER.to_string(),
            version: SCHEMA_VERSION,
            revision,
            payload,
        }
    }

    /// Validate the envelope header.
    pub fn validate_header(&self) -> Result<(), StoreError> {
        if self.schema != SCHEMA_IDENTIFIER {
            return Err(StoreError::UnsupportedSchema(self.schema.clone()));
        }
        if self.version != SCHEMA_VERSION {
            return Err(StoreError::UnsupportedVersion(self.version));
        }
        Ok(())
    }
}

/// Errors from store operations.
#[derive(Debug, Clone)]
pub enum StoreError {
    /// The schema is not recognized.
    UnsupportedSchema(String),

    /// The schema version is not supported.
    UnsupportedVersion(u32),

    /// Serialization/deserialization error.
    Serialization(String),

    /// Filesystem error.
    Io(String),

    /// The generation file is corrupt or incomplete.
    CorruptGeneration(PathBuf, String),

    /// All generations are corrupt; no valid state can be loaded.
    AllCorrupt(String),

    /// State files exist but no valid generation was found.
    NoValidGeneration(String),

    /// Path escape detected.
    PathEscape(String),

    /// State is oversized.
    Oversized { limit: usize, actual: usize },

    /// The store is in an invalid state.
    InvalidState(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema(s) => write!(f, "unsupported schema: {}", s),
            Self::UnsupportedVersion(v) => write!(f, "unsupported schema version: {}", v),
            Self::Serialization(e) => write!(f, "serialization error: {}", e),
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::CorruptGeneration(path, e) => {
                write!(f, "corrupt generation {}: {}", path.display(), e)
            }
            Self::AllCorrupt(msg) => write!(f, "all generations corrupt: {}", msg),
            Self::NoValidGeneration(msg) => {
                write!(f, "no valid generation found: {}", msg)
            }
            Self::PathEscape(msg) => write!(f, "path escape: {}", msg),
            Self::Oversized { limit, actual } => {
                write!(
                    f,
                    "state oversized: limit {} bytes, actual {} bytes",
                    limit, actual
                )
            }
            Self::InvalidState(msg) => write!(f, "invalid state: {}", msg),
        }
    }
}

impl std::error::Error for StoreError {}

/// Result type for store operations.
pub type StoreResult<T> = Result<T, StoreError>;

/// Generic versioned generation store.
///
/// Provides restart-safe persistence with atomic publication, corruption
/// fallback, and bounded retention.
pub struct GenerationStore<T> {
    /// The directory containing generation files.
    dir: PathBuf,

    /// The current in-memory snapshot.
    current: Option<T>,

    /// The current revision.
    revision: StateRevision,

    /// Maximum allowed serialized size in bytes.
    max_size: usize,
}

impl<T> GenerationStore<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync,
{
    /// Create a new generation store for the given directory.
    ///
    /// Does not load existing state; call `load` to initialize from disk.
    pub fn new(dir: PathBuf, max_size: usize) -> Self {
        Self {
            dir,
            current: None,
            revision: StateRevision::ZERO,
            max_size,
        }
    }

    /// Return the current in-memory state, if any.
    pub fn current(&self) -> Option<&T> {
        self.current.as_ref()
    }

    /// Return the current revision.
    pub fn revision(&self) -> StateRevision {
        self.revision
    }

    /// Return the store directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Publish a new state generation.
    ///
    /// 1. Validates the new state via the provided validator.
    /// 2. Increments the revision.
    /// 3. Serializes deterministically.
    /// 4. Writes to a temporary file.
    /// 5. Renames to the final generation path.
    /// 6. Updates the in-memory snapshot.
    pub async fn publish<F>(&mut self, state: T, validate: F) -> StoreResult<StateRevision>
    where
        F: FnOnce(&T) -> Result<(), StoreError>,
    {
        // Validate before any writes
        validate(&state)?;

        let new_revision = self.revision.next();

        // Serialize deterministically
        let envelope = Envelope::new(new_revision, &state);
        let json =
            serde_json::to_vec(&envelope).map_err(|e| StoreError::Serialization(e.to_string()))?;

        // Check size limit
        if json.len() > self.max_size {
            return Err(StoreError::Oversized {
                limit: self.max_size,
                actual: json.len(),
            });
        }

        // Ensure directory exists
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| StoreError::Io(e.to_string()))?;

        // Generate unique filename
        let gen_name = format!("gen-{:020}.json", new_revision.value());
        let temp_name = format!(".tmp-{}", gen_name);
        let temp_path = self.dir.join(&temp_name);
        let final_path = self.dir.join(&gen_name);

        // Write to temporary file
        tokio::fs::write(&temp_path, &json)
            .await
            .map_err(|e| StoreError::Io(e.to_string()))?;

        // Rename to final path (atomic on same filesystem)
        tokio::fs::rename(&temp_path, &final_path)
            .await
            .map_err(|e| StoreError::Io(e.to_string()))?;

        // Update in-memory state
        self.current = Some(state);
        self.revision = new_revision;

        // Cleanup old generations (best effort)
        self.cleanup().await;

        Ok(new_revision)
    }

    /// Load the newest valid generation from disk.
    ///
    /// Scans generation files newest-first, falling back to prior valid
    /// generations on corruption.
    pub async fn load(&mut self) -> StoreResult<Option<StateRevision>> {
        // Ensure directory exists
        if !self.dir.exists() {
            return Ok(None);
        }

        // Collect generation files
        let mut entries: Vec<PathBuf> = Vec::new();
        let mut dir_entries = match tokio::fs::read_dir(&self.dir).await {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(StoreError::Io(e.to_string())),
        };

        while let Some(entry) =
            dir_entries.next_entry().await.map_err(|e| StoreError::Io(e.to_string()))?
        {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                entries.push(path);
            }
            if entries.len() >= MAX_GENERATION_SCAN {
                break;
            }
        }

        if entries.is_empty() {
            return Ok(None);
        }

        // Sort by filename (generation numbers sort correctly as strings
        // due to zero-padding)
        entries.sort();
        entries.reverse(); // newest first

        // Try each generation, newest first
        let mut last_error = None;
        for path in &entries {
            match self.try_load_generation(path).await {
                Ok(revision) => {
                    tracing::info!(
                        "loaded generation {:?} at revision {}",
                        path.file_name(),
                        revision
                    );
                    return Ok(Some(revision));
                }
                Err(e) => {
                    tracing::warn!("failed to load generation {:?}: {}", path.file_name(), e);
                    last_error = Some(e);
                }
            }
        }

        // All generations failed
        if entries.len() > 1 {
            Err(StoreError::AllCorrupt(format!(
                "all {} generation files are corrupt",
                entries.len()
            )))
        } else {
            Err(last_error.unwrap_or_else(|| {
                StoreError::NoValidGeneration("no valid generation found".to_string())
            }))
        }
    }

    /// Try to load a single generation file.
    async fn try_load_generation(&mut self, path: &Path) -> StoreResult<StateRevision> {
        let json = tokio::fs::read(path).await.map_err(|e| StoreError::Io(e.to_string()))?;

        let envelope: Envelope<T> = serde_json::from_slice(&json)
            .map_err(|e| StoreError::CorruptGeneration(path.to_path_buf(), e.to_string()))?;

        envelope
            .validate_header()
            .map_err(|e| StoreError::CorruptGeneration(path.to_path_buf(), e.to_string()))?;

        self.current = Some(envelope.payload);
        self.revision = envelope.revision;

        Ok(envelope.revision)
    }

    /// Cleanup old generations, keeping at most MAX_RETENTION prior good
    /// generations plus the current one.
    async fn cleanup(&self) {
        let mut entries: Vec<PathBuf> = Vec::new();
        if let Ok(mut dir_entries) = tokio::fs::read_dir(&self.dir).await {
            while let Ok(Some(entry)) = dir_entries.next_entry().await {
                let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                    entries.push(path);
                }
            }
        }

        if entries.len() <= MAX_RETENTION + 1 {
            return;
        }

        entries.sort();
        // Keep the newest MAX_RETENTION + 1 files, delete the rest
        let to_delete = entries.len() - MAX_RETENTION - 1;
        for path in entries.iter().take(to_delete) {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestPayload {
        value: String,
    }

    fn test_store_dir() -> PathBuf {
        tempfile::tempdir().unwrap().keep()
    }

    #[tokio::test]
    async fn publish_and_load() {
        let dir = test_store_dir();
        let mut store = GenerationStore::<TestPayload>::new(dir.clone(), 1024 * 1024);

        let payload = TestPayload {
            value: "hello".to_string(),
        };
        let revision = store.publish(payload.clone(), |_| Ok(())).await.unwrap();
        assert_eq!(revision, StateRevision::new(1));

        // Load from disk
        let mut store2 = GenerationStore::<TestPayload>::new(dir.clone(), 1024 * 1024);
        let loaded = store2.load().await.unwrap();
        assert_eq!(loaded, Some(StateRevision::new(1)));
        assert_eq!(store2.current(), Some(&payload));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn load_empty_dir() {
        let dir = test_store_dir();
        let mut store = GenerationStore::<TestPayload>::new(dir.clone(), 1024 * 1024);
        let loaded = store.load().await.unwrap();
        assert_eq!(loaded, None);
        assert!(store.current().is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn validation_rejects_before_write() {
        let dir = test_store_dir();
        let mut store = GenerationStore::<TestPayload>::new(dir.clone(), 1024 * 1024);

        let payload = TestPayload {
            value: "bad".to_string(),
        };
        let result = store
            .publish(payload, |_| {
                Err(StoreError::InvalidState("test rejection".to_string()))
            })
            .await;

        assert!(result.is_err());
        assert!(store.current().is_none());
        assert_eq!(store.revision(), StateRevision::ZERO);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn oversized_rejected() {
        let dir = test_store_dir();
        let mut store = GenerationStore::<TestPayload>::new(dir.clone(), 10); // tiny limit

        let payload = TestPayload {
            value: "this is way too long".to_string(),
        };
        let result = store.publish(payload, |_| Ok(())).await;

        assert!(matches!(result, Err(StoreError::Oversized { .. })));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn revision_increments() {
        let dir = test_store_dir();
        let mut store = GenerationStore::<TestPayload>::new(dir.clone(), 1024 * 1024);

        let r1 = store
            .publish(
                TestPayload {
                    value: "first".to_string(),
                },
                |_| Ok(()),
            )
            .await
            .unwrap();
        let r2 = store
            .publish(
                TestPayload {
                    value: "second".to_string(),
                },
                |_| Ok(()),
            )
            .await
            .unwrap();

        assert!(r2 > r1);
        assert_eq!(r1, StateRevision::new(1));
        assert_eq!(r2, StateRevision::new(2));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn envelope_validate_header() {
        let envelope = Envelope::new(
            StateRevision::ZERO,
            TestPayload {
                value: "test".to_string(),
            },
        );
        assert!(envelope.validate_header().is_ok());

        let bad_schema = Envelope {
            schema: "wrong".to_string(),
            ..envelope.clone()
        };
        assert!(matches!(
            bad_schema.validate_header(),
            Err(StoreError::UnsupportedSchema(_))
        ));

        let bad_version = Envelope {
            version: 999,
            ..envelope
        };
        assert!(matches!(
            bad_version.validate_header(),
            Err(StoreError::UnsupportedVersion(999))
        ));
    }
}
