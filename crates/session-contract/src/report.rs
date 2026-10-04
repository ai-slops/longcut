use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conformance {
    Conformant,
    Degraded,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Capabilities {
    pub program_reference: bool,
    pub independent_video_sources: bool,
    pub independent_audio_sources: bool,
    pub session_time_map: bool,
    pub semantic_visual_reconstruction: bool,
    pub semantic_audio_reconstruction: bool,
    pub media_health_ranges: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub metadata_conformance: Conformance,
    pub media_verification: &'static str,
    pub capabilities: Capabilities,
    pub diagnostics: Vec<Diagnostic>,
}

impl Default for Report {
    fn default() -> Self {
        Self {
            metadata_conformance: Conformance::Conformant,
            media_verification: "not_performed",
            capabilities: Capabilities::default(),
            diagnostics: Vec::new(),
        }
    }
}

impl Report {
    pub(crate) fn error(
        &mut self,
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.metadata_conformance = Conformance::Rejected;
        self.capabilities = Capabilities::default();
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code,
            path: path.into(),
            message: message.into(),
        });
    }

    pub(crate) fn warning(
        &mut self,
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        if self.metadata_conformance == Conformance::Conformant {
            self.metadata_conformance = Conformance::Degraded;
        }
        self.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code,
            path: path.into(),
            message: message.into(),
        });
    }

    pub(crate) fn finish(mut self) -> Self {
        if self.metadata_conformance == Conformance::Rejected {
            self.capabilities = Capabilities::default();
        }
        self
    }
}
