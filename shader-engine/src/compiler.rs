//! Shader source loading and `naga`-based compilation and validation.

use crate::ShaderError;
use naga::{
    Module,
    valid::{Capabilities, ValidationFlags, Validator},
};
use std::path::{Path, PathBuf};

/// Shader source language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShaderLanguage {
    /// WebGPU Shading Language (preferred).
    Wgsl,
    /// OpenGL Shading Language (transpiled via naga).
    Glsl,
}

/// Raw, unvalidated shader source.
#[derive(Debug, Clone)]
pub struct ShaderSource {
    /// Language of this source.
    pub language: ShaderLanguage,
    /// Source text.
    pub code: String,
    /// Optional originating file path for diagnostics.
    pub path: Option<PathBuf>,
}

impl ShaderSource {
    /// Load from a file, detecting language from the extension.
    ///
    /// # Errors
    /// Returns [`ShaderError::Io`] on read failure.
    pub fn from_file(path: &Path) -> Result<Self, ShaderError> {
        let code = std::fs::read_to_string(path).map_err(|e| ShaderError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        let language = match path.extension().and_then(|e| e.to_str()) {
            Some("glsl" | "frag" | "vert") => ShaderLanguage::Glsl,
            _ => ShaderLanguage::Wgsl,
        };
        Ok(Self {
            language,
            code,
            path: Some(path.to_path_buf()),
        })
    }

    /// Create an inline WGSL source.
    #[must_use]
    pub fn from_wgsl(code: impl Into<String>) -> Self {
        Self {
            language: ShaderLanguage::Wgsl,
            code: code.into(),
            path: None,
        }
    }
}

/// A naga-validated shader module, ready for GPU pipeline creation.
#[derive(Debug)]
pub struct CompiledShader {
    /// The validated naga IR.
    pub module: Module,
    /// Original source (retained for diagnostics and diffing).
    pub source: ShaderSource,
}

/// Stateless compiler that wraps naga parsing and validation.
#[derive(Debug, Default)]
pub struct ShaderCompiler;

impl ShaderCompiler {
    /// Parse and validate `source`, returning a [`CompiledShader`] on success.
    ///
    /// # Errors
    /// Returns [`ShaderError::Parse`] or [`ShaderError::Validation`] on failure.
    pub fn compile(&self, source: ShaderSource) -> Result<CompiledShader, ShaderError> {
        let label = source
            .path
            .as_deref()
            .map_or_else(|| "<inline>".to_owned(), |p| p.display().to_string());

        let module = match source.language {
            ShaderLanguage::Wgsl => {
                naga::front::wgsl::parse_str(&source.code).map_err(|e| ShaderError::Parse {
                    file: label.clone(),
                    detail: e.to_string(),
                })?
            }
            ShaderLanguage::Glsl => {
                let mut parser = naga::front::glsl::Frontend::default();
                parser
                    .parse(
                        &naga::front::glsl::Options::from(naga::ShaderStage::Fragment),
                        &source.code,
                    )
                    .map_err(|e| ShaderError::Parse {
                        file: label.clone(),
                        detail: format!("{e:?}"),
                    })?
            }
        };

        let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
        validator
            .validate(&module)
            .map_err(|e| ShaderError::Validation {
                file: label,
                detail: e.to_string(),
            })?;

        Ok(CompiledShader { module, source })
    }
}
