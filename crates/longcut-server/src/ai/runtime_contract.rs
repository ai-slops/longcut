use super::{AiProvider, RuntimeInfo, RuntimeJob, RuntimeJobStatus, RuntimePhase};
use crate::error::{AppError, AppResult};

const RUNTIME_ENGINE: &str = "@slop-lab/part";
const RUNTIME_VERSION: &str = "0.1.0";
const RUNTIME_PROTOCOL_VERSION: u8 = 1;

pub(crate) fn validate_runtime_info(info: RuntimeInfo) -> AppResult<RuntimeInfo> {
    if info.engine != RUNTIME_ENGINE
        || info.version != RUNTIME_VERSION
        || info.node_version.trim().is_empty()
        || info.protocol_version != RUNTIME_PROTOCOL_VERSION
    {
        return Err(invalid_runtime_response());
    }
    Ok(info)
}

pub(super) fn validate_runtime_job(
    job: RuntimeJob,
    expected_id: &str,
    expected_provider: Option<AiProvider>,
    previous: Option<&RuntimeJob>,
    terminal_required: bool,
) -> AppResult<RuntimeJob> {
    if job.id != expected_id {
        return Err(AppError::Conflict(
            "Agent runtime job identity mismatch".into(),
        ));
    }
    let expected_profile = expected_provider.map(profile_revision);
    let profile_is_valid = match expected_profile {
        Some(profile) => job.runtime.profile_revision == profile,
        None => matches!(
            job.runtime.profile_revision.as_str(),
            "longcut-codex-v1" | "longcut-openai-compatible-v1"
        ),
    };
    if job.runtime.engine != RUNTIME_ENGINE
        || job.runtime.engine_version != RUNTIME_VERSION
        || job.runtime.execution_id.trim().is_empty()
        || !profile_is_valid
    {
        return Err(invalid_runtime_response());
    }
    if let Some(previous) = previous
        && (job.runtime.execution_id != previous.runtime.execution_id
            || job.progress.sequence < previous.progress.sequence
            || phase_order(job.progress.phase) < phase_order(previous.progress.phase))
    {
        return Err(invalid_runtime_response());
    }
    let shape_is_valid = match job.status {
        RuntimeJobStatus::Running => {
            job.progress.phase != RuntimePhase::Terminal
                && job.result.is_none()
                && job.error.is_none()
        }
        RuntimeJobStatus::Succeeded => {
            job.progress.phase == RuntimePhase::Terminal
                && job.result.is_some()
                && job.error.is_none()
        }
        RuntimeJobStatus::Failed => {
            job.progress.phase == RuntimePhase::Terminal
                && job.result.is_none()
                && job.error.is_some()
        }
        RuntimeJobStatus::Cancelled => {
            job.progress.phase == RuntimePhase::Terminal && job.result.is_none()
        }
    };
    if !shape_is_valid || (terminal_required && job.status == RuntimeJobStatus::Running) {
        return Err(invalid_runtime_response());
    }
    Ok(job)
}

pub(super) fn invalid_runtime_response() -> AppError {
    AppError::Conflict("Agent runtime returned an invalid response".into())
}

const fn profile_revision(provider: AiProvider) -> &'static str {
    match provider {
        AiProvider::Codex => "longcut-codex-v1",
        AiProvider::OpenaiCompatible => "longcut-openai-compatible-v1",
    }
}

const fn phase_order(phase: RuntimePhase) -> u8 {
    match phase {
        RuntimePhase::Admitted => 0,
        RuntimePhase::Provider => 1,
        RuntimePhase::Validating => 2,
        RuntimePhase::CleaningUp => 3,
        RuntimePhase::Terminal => 4,
    }
}
