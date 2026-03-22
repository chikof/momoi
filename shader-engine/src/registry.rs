//! Thread-safe registry mapping names to compiled shaders.
//!
//! When the `compile` feature is disabled, only raw source strings are stored.

use crate::ShaderError;
use parking_lot::RwLock;
use std::{collections::HashMap, path::Path, sync::Arc};
use tracing::{debug, info};

/// Unique numeric shader identifier.
pub type ShaderId = u64;

fn next_id() -> ShaderId {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Raw shader source entry (always stored regardless of features).
#[derive(Debug, Clone)]
pub struct ShaderEntry {
    /// The shader source code (WGSL or GLSL).
    pub source: String,
    /// Optional file path (used for hot-reload and error messages).
    pub path: Option<std::path::PathBuf>,
}

#[derive(Debug, Default)]
struct RegistryInner {
    entries: HashMap<ShaderId, Arc<ShaderEntry>>,
    names: HashMap<String, ShaderId>,
}

/// Shared, thread-safe store of shader sources.
///
/// When the `compile` feature is enabled, shaders are also validated
/// by `naga` at registration time.
#[derive(Debug, Default, Clone)]
pub struct ShaderRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

impl ShaderRegistry {
    /// Register a shader from a file path, auto-detecting the language.
    ///
    /// # Errors
    /// Returns [`ShaderError::Io`] on read failure.
    /// Returns [`ShaderError::Parse`] or [`ShaderError::Validation`] when the
    /// `compile` feature is enabled and validation fails.
    pub fn register_file(&self, name: &str, path: &Path) -> Result<ShaderId, ShaderError> {
        let source = std::fs::read_to_string(path).map_err(|e| ShaderError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        self.register_raw(name, source, Some(path.to_path_buf()))
    }

    /// Register an inline WGSL shader source string.
    ///
    /// # Errors
    /// Returns [`ShaderError::Parse`] or [`ShaderError::Validation`] when the
    /// `compile` feature is enabled and validation fails.
    pub fn register_wgsl(&self, name: &str, code: &str) -> Result<ShaderId, ShaderError> {
        self.register_raw(name, code.to_owned(), None)
    }

    fn register_raw(
        &self,
        name: &str,
        source: String,
        path: Option<std::path::PathBuf>,
    ) -> Result<ShaderId, ShaderError> {
        // Validate source when the compile feature is active.
        #[cfg(feature = "compile")]
        {
            use crate::compiler::{ShaderCompiler, ShaderLanguage, ShaderSource};
            let lang = path
                .as_deref()
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
                .map_or(ShaderLanguage::Wgsl, |ext| match ext {
                    "glsl" | "frag" | "vert" => ShaderLanguage::Glsl,
                    _ => ShaderLanguage::Wgsl,
                });
            let src = ShaderSource {
                language: lang,
                code: source.clone(),
                path: path.clone(),
            };
            ShaderCompiler.compile(src)?;
        }

        let id = next_id();
        let entry = Arc::new(ShaderEntry { source, path });
        let mut inner = self.inner.write();
        inner.names.insert(name.to_owned(), id);
        inner.entries.insert(id, entry);
        info!(shader = name, id, "shader registered");
        Ok(id)
    }

    /// Retrieve a shader entry by its numeric ID.
    #[must_use]
    pub fn get(&self, id: ShaderId) -> Option<Arc<ShaderEntry>> {
        self.inner.read().entries.get(&id).cloned()
    }

    /// Retrieve a shader entry by its human-readable name.
    #[must_use]
    pub fn get_by_name(&self, name: &str) -> Option<Arc<ShaderEntry>> {
        let inner = self.inner.read();
        inner
            .names
            .get(name)
            .and_then(|id| inner.entries.get(id))
            .cloned()
    }

    /// Replace an existing shader entry (called on hot-reload).
    ///
    /// # Errors
    /// Same as [`Self::register_file`].
    pub fn reload(&self, name: &str, path: &Path) -> Result<ShaderId, ShaderError> {
        debug!(shader = name, "hot-reloading shader");
        self.register_file(name, path)
    }
}
