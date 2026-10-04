use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::{Conformance, Report, model::*, validate};

const MAX_METADATA_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Error)]
enum LoadError {
    #[error("unsafe package-relative path")]
    UnsafePath,
    #[error("metadata must be a regular file, with no symlink components")]
    UnsafeFile,
    #[error("metadata exceeds 4 MiB limit")]
    TooLarge,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub(crate) fn safe_relative(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['\\', ':', '%', '?', '#'])
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn read_json<T: DeserializeOwned>(root: &Path, relative: &str) -> Result<T, LoadError> {
    if !safe_relative(relative) {
        return Err(LoadError::UnsafePath);
    }
    let mut path = root.to_path_buf();
    for component in relative.split('/') {
        path.push(component);
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(LoadError::UnsafeFile);
        }
    }
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(LoadError::UnsafeFile);
    }
    if metadata.len() > MAX_METADATA_BYTES {
        return Err(LoadError::TooLarge);
    }
    let mut bytes = Vec::new();
    File::open(&path)?
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_err(|_| LoadError::TooLarge)? > MAX_METADATA_BYTES {
        return Err(LoadError::TooLarge);
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn optional<T: DeserializeOwned>(
    root: &Path,
    name: Option<&str>,
    report: &mut Report,
) -> Option<T> {
    let name = name?;
    match read_json(root, name) {
        Ok(value) => Some(value),
        Err(LoadError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            report.warning(
                "METADATA_FILE_MISSING",
                name,
                "declared optional track file is missing",
            );
            None
        }
        Err(error) => {
            report.error("INVALID_METADATA_FILE", name, error.to_string());
            None
        }
    }
}

/// Inspect immutable local metadata only. No media files, URLs or producer extensions are executed.
pub fn inspect(root: &Path) -> Report {
    match load(root) {
        Ok(loaded) => loaded.report,
        Err(report) => report,
    }
}

#[derive(Debug)]
pub struct LoadedPackage {
    package: Package,
    report: Report,
}

impl LoadedPackage {
    pub fn into_parts(self) -> (Package, Report) {
        (self.package, self.report)
    }
}

/// Return metadata only after the same validation gate used by `inspect` accepts it.
pub fn load(root: &Path) -> Result<LoadedPackage, Report> {
    let mut report = Report::default();
    let manifest: Manifest = match read_json(root, "manifest.json") {
        Ok(value) => value,
        Err(error) => {
            report.error("INVALID_MANIFEST", "manifest.json", error.to_string());
            return Err(report);
        }
    };
    let registry = match read_json(root, &manifest.sources_file) {
        Ok(value) => value,
        Err(error) => {
            report.error(
                "INVALID_REGISTRY",
                &manifest.sources_file,
                error.to_string(),
            );
            return Err(report);
        }
    };
    let package = Package {
        registry,
        time_maps: optional(root, manifest.time_maps_file.as_deref(), &mut report),
        visual_composition: optional(
            root,
            manifest.visual_composition_file.as_deref(),
            &mut report,
        ),
        audio_mix: optional(root, manifest.audio_mix_file.as_deref(), &mut report),
        media_health: optional(root, manifest.media_health_file.as_deref(), &mut report),
        manifest,
    };
    let mut validation = validate(&package);
    for diagnostic in report.diagnostics {
        match diagnostic.severity {
            crate::Severity::Error => {
                validation.error(diagnostic.code, diagnostic.path, diagnostic.message)
            }
            crate::Severity::Warning => {
                validation.warning(diagnostic.code, diagnostic.path, diagnostic.message)
            }
        }
    }
    let report = validation.finish();
    match report.metadata_conformance {
        Conformance::Rejected => Err(report),
        Conformance::Conformant | Conformance::Degraded => Ok(LoadedPackage { package, report }),
    }
}
