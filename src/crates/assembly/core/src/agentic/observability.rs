use crate::util::errors::BitFunError;
use bitfun_core_types::errors::ErrorCategory;
use bitfun_events::{
    SafeCountBucket, SafeOperationErrorType, SafeOperationOutcome, SafePermissionDecision,
    SafePermissionSource, SafeSessionClass, SafeSessionOperation,
};
use bitfun_observability::domains::{
    AgentModeClass, CompletionFacts, CountBucket, FinishReasonClass, InferenceProtocolClass,
    ModelClass, Outcome, PermissionDecision, PermissionSource, ProviderClass, SafeErrorType,
    SessionClass, SessionOperation, StatusClass, ToolClass, ToolKind, ToolSourceClass, TurnTrigger,
};

pub(crate) fn safe_terminal_completion(
    completion: CompletionFacts,
) -> (SafeOperationOutcome, Option<SafeOperationErrorType>) {
    let outcome = match completion.outcome() {
        Outcome::Completed => SafeOperationOutcome::Completed,
        Outcome::Failed => SafeOperationOutcome::Failed,
        Outcome::Cancelled => SafeOperationOutcome::Cancelled,
        Outcome::Timeout => SafeOperationOutcome::Timeout,
        Outcome::Rejected => SafeOperationOutcome::Rejected,
        Outcome::Degraded => SafeOperationOutcome::Degraded,
        Outcome::Incomplete => SafeOperationOutcome::Incomplete,
    };
    let error_type = completion.error_type().map(|error_type| match error_type {
        SafeErrorType::Cancelled => SafeOperationErrorType::Cancelled,
        SafeErrorType::Timeout => SafeOperationErrorType::Timeout,
        SafeErrorType::Authentication => SafeOperationErrorType::Authentication,
        SafeErrorType::RateLimited => SafeOperationErrorType::RateLimited,
        SafeErrorType::NetworkUnavailable => SafeOperationErrorType::NetworkUnavailable,
        SafeErrorType::NetworkProtocol => SafeOperationErrorType::NetworkProtocol,
        SafeErrorType::InvalidRequest => SafeOperationErrorType::InvalidRequest,
        SafeErrorType::ContextOverflow => SafeOperationErrorType::ContextOverflow,
        SafeErrorType::ToolValidation => SafeOperationErrorType::ToolValidation,
        SafeErrorType::PermissionDenied => SafeOperationErrorType::PermissionDenied,
        SafeErrorType::Persistence => SafeOperationErrorType::Persistence,
        SafeErrorType::Provider => SafeOperationErrorType::Provider,
        SafeErrorType::Internal => SafeOperationErrorType::Internal,
        SafeErrorType::Other => SafeOperationErrorType::Other,
    });
    (outcome, error_type)
}

pub(crate) fn safe_session_operation(value: SessionOperation) -> SafeSessionOperation {
    match value {
        SessionOperation::Create => SafeSessionOperation::Create,
        SessionOperation::Resume => SafeSessionOperation::Resume,
        SessionOperation::Delete => SafeSessionOperation::Delete,
    }
}

pub(crate) fn safe_session_class(value: SessionClass) -> SafeSessionClass {
    match value {
        SessionClass::Standard => SafeSessionClass::Standard,
        SessionClass::Subagent => SafeSessionClass::Subagent,
        SessionClass::Internal => SafeSessionClass::Internal,
        SessionClass::Transient => SafeSessionClass::Transient,
    }
}

pub(crate) fn safe_count_bucket(value: CountBucket) -> SafeCountBucket {
    match value {
        CountBucket::Zero => SafeCountBucket::Zero,
        CountBucket::One => SafeCountBucket::One,
        CountBucket::Two => SafeCountBucket::Two,
        CountBucket::ThreePlus => SafeCountBucket::ThreePlus,
    }
}

pub(crate) fn safe_permission_decision(value: PermissionDecision) -> SafePermissionDecision {
    match value {
        PermissionDecision::Allow => SafePermissionDecision::Allow,
        PermissionDecision::Ask => SafePermissionDecision::Ask,
        PermissionDecision::PolicyDeny => SafePermissionDecision::PolicyDeny,
        PermissionDecision::UserReject => SafePermissionDecision::UserReject,
        PermissionDecision::Cancelled => SafePermissionDecision::Cancelled,
        PermissionDecision::Failed => SafePermissionDecision::Failed,
    }
}

pub(crate) fn safe_permission_source(value: PermissionSource) -> SafePermissionSource {
    match value {
        PermissionSource::Policy => SafePermissionSource::Policy,
        PermissionSource::StoredGrant => SafePermissionSource::StoredGrant,
        PermissionSource::Hook => SafePermissionSource::Hook,
        PermissionSource::AutoApprove => SafePermissionSource::AutoApprove,
        PermissionSource::User => SafePermissionSource::User,
        PermissionSource::Delegated => SafePermissionSource::Delegated,
        PermissionSource::Other => SafePermissionSource::Other,
    }
}

pub(crate) fn completion_from_error(error: &BitFunError) -> CompletionFacts {
    match error {
        BitFunError::Cancelled(_) => CompletionFacts::cancelled(),
        BitFunError::Timeout(_) => CompletionFacts::timeout(),
        BitFunError::Validation(_) => CompletionFacts::failed(SafeErrorType::InvalidRequest),
        BitFunError::AIProvider(error) => {
            CompletionFacts::failed(safe_error_category(&error.category))
        }
        BitFunError::RecoverableContextOverflow(_) => {
            CompletionFacts::failed(SafeErrorType::ContextOverflow)
        }
        BitFunError::AIClient(_) => CompletionFacts::failed(SafeErrorType::Provider),
        BitFunError::Io(_) => CompletionFacts::failed(SafeErrorType::Persistence),
        BitFunError::Http(_) => CompletionFacts::failed(SafeErrorType::NetworkProtocol),
        BitFunError::Configuration(_) | BitFunError::Deserialization(_) => {
            CompletionFacts::failed(SafeErrorType::InvalidRequest)
        }
        BitFunError::Tool(_) | BitFunError::NotFound(_) => {
            CompletionFacts::failed(SafeErrorType::ToolValidation)
        }
        _ => CompletionFacts::failed(SafeErrorType::Other),
    }
}

pub(crate) fn tool_completion_from_error(error: &BitFunError) -> CompletionFacts {
    match error {
        BitFunError::Tool(_) | BitFunError::Validation(_) | BitFunError::NotFound(_) => {
            CompletionFacts::failed(SafeErrorType::ToolValidation)
        }
        _ => completion_from_error(error),
    }
}

pub(crate) fn safe_error_category(category: &ErrorCategory) -> SafeErrorType {
    match category {
        ErrorCategory::Network => SafeErrorType::NetworkUnavailable,
        ErrorCategory::Auth => SafeErrorType::Authentication,
        ErrorCategory::RateLimit => SafeErrorType::RateLimited,
        ErrorCategory::ContextOverflow => SafeErrorType::ContextOverflow,
        ErrorCategory::Timeout => SafeErrorType::Timeout,
        ErrorCategory::Permission => SafeErrorType::PermissionDenied,
        ErrorCategory::InvalidRequest => SafeErrorType::InvalidRequest,
        ErrorCategory::ProviderQuota
        | ErrorCategory::ProviderBilling
        | ErrorCategory::ProviderUnavailable
        | ErrorCategory::ContentPolicy
        | ErrorCategory::ModelError => SafeErrorType::Provider,
        ErrorCategory::Unknown => SafeErrorType::Other,
    }
}

pub(crate) fn retryable_error(error: &BitFunError) -> bool {
    match error {
        BitFunError::AIProvider(error) => matches!(
            error.category,
            ErrorCategory::Network
                | ErrorCategory::RateLimit
                | ErrorCategory::Timeout
                | ErrorCategory::ProviderUnavailable
        ),
        BitFunError::Timeout(_) | BitFunError::Http(_) | BitFunError::Io(_) => true,
        _ => false,
    }
}

pub(crate) fn status_class(error: Option<&BitFunError>) -> StatusClass {
    let status = match error {
        Some(BitFunError::AIProvider(error))
        | Some(BitFunError::RecoverableContextOverflow(error)) => error.http_status,
        Some(BitFunError::Http(_)) | Some(BitFunError::Io(_)) => return StatusClass::Network,
        Some(_) => return StatusClass::None,
        None => return StatusClass::Success,
    };
    match status {
        Some(200..=299) => StatusClass::Success,
        Some(300..=399) => StatusClass::Redirect,
        Some(400..=499) => StatusClass::ClientError,
        Some(500..=599) => StatusClass::ServerError,
        _ => StatusClass::None,
    }
}

pub(crate) fn agent_mode_class(agent_type: &str) -> AgentModeClass {
    match agent_type.to_ascii_lowercase().as_str() {
        "agent" | "agentic" | "build" | "plan" => AgentModeClass::Agentic,
        "chat" => AgentModeClass::Chat,
        "review" | "deepreview" | "deep_review" => AgentModeClass::Review,
        "goal" => AgentModeClass::Goal,
        _ => AgentModeClass::Custom,
    }
}

pub(crate) fn turn_trigger(is_subagent: bool, remote: bool) -> TurnTrigger {
    if is_subagent {
        TurnTrigger::Continuation
    } else if remote {
        TurnTrigger::Remote
    } else {
        TurnTrigger::User
    }
}

pub(crate) fn finish_reason_class(value: &str) -> FinishReasonClass {
    match value {
        "complete" | "completed" => FinishReasonClass::Completed,
        "tool_calls" => FinishReasonClass::ToolCalls,
        "cancelled" => FinishReasonClass::Cancelled,
        "length" => FinishReasonClass::Length,
        "content_filter" => FinishReasonClass::ContentFilter,
        "max_rounds" => FinishReasonClass::MaxRounds,
        "repeated_tool_failures" => FinishReasonClass::RepeatedToolFailures,
        "error" => FinishReasonClass::Error,
        _ => FinishReasonClass::Other,
    }
}

pub(crate) fn inference_classes(
    format: &str,
    model: &str,
) -> (ProviderClass, ModelClass, InferenceProtocolClass) {
    let format = format.to_ascii_lowercase();
    let protocol_class = match format.as_str() {
        "responses" => InferenceProtocolClass::Responses,
        "openai" => InferenceProtocolClass::ChatCompletions,
        "anthropic" => InferenceProtocolClass::Messages,
        "gemini" | "google" => InferenceProtocolClass::Gemini,
        _ => InferenceProtocolClass::Other,
    };
    let provider_class = match format.as_str() {
        "responses" | "openai" => ProviderClass::OpenAiCompatible,
        "anthropic" => ProviderClass::AnthropicCompatible,
        "gemini" | "google" => ProviderClass::GoogleCompatible,
        "ollama" | "local" => ProviderClass::Local,
        _ => ProviderClass::Other,
    };
    let model = model.to_ascii_lowercase();
    let model_class = if model.contains("vision") || model.contains("vl") {
        ModelClass::Vision
    } else if model.contains("code") || model.contains("codex") {
        ModelClass::Code
    } else if model.contains("flash") || model.contains("mini") || model.contains("haiku") {
        ModelClass::Fast
    } else if model.is_empty() {
        ModelClass::Other
    } else {
        ModelClass::GeneralReasoning
    };
    (provider_class, model_class, protocol_class)
}

pub(crate) fn tool_identity(
    tool_name: &str,
    provider_kind: Option<&str>,
) -> (ToolClass, ToolSourceClass, ToolKind) {
    let normalized_name = tool_name.to_ascii_lowercase();
    let source = match provider_kind.map(str::to_ascii_lowercase).as_deref() {
        Some("mcp") => ToolSourceClass::Mcp,
        Some("external_source" | "external") => ToolSourceClass::External,
        Some("plugin" | "opencode" | "extension") => ToolSourceClass::Plugin,
        Some("builtin" | "static") => ToolSourceClass::BuiltIn,
        Some(_) => ToolSourceClass::Custom,
        None if normalized_name == "skill" => ToolSourceClass::Skill,
        None => ToolSourceClass::BuiltIn,
    };
    let kind = match normalized_name.as_str() {
        "read" | "write" | "edit" | "multiedit" | "glob" | "list" => ToolKind::Filesystem,
        "grep" | "search" | "websearch" | "codesearch" => ToolKind::Search,
        "bash" | "shell" | "terminal" | "executecommand" => ToolKind::Shell,
        "git" | "gitstatus" | "gitdiff" => ToolKind::Git,
        "browser" | "webfetch" | "webdriver" => ToolKind::Browser,
        "computeruse" | "computer_use" => ToolKind::ComputerUse,
        "mcp" | "calldeferredtool" | "gettoolspec" => ToolKind::Protocol,
        "task" | "subagent" | "createsubagent" => ToolKind::Task,
        _ => ToolKind::Other,
    };
    let kind = if source == ToolSourceClass::Mcp && kind == ToolKind::Other {
        ToolKind::Protocol
    } else {
        kind
    };
    let class = match source {
        ToolSourceClass::BuiltIn | ToolSourceClass::Skill => ToolClass::BuiltIn,
        ToolSourceClass::Mcp
        | ToolSourceClass::Plugin
        | ToolSourceClass::External
        | ToolSourceClass::Custom => ToolClass::Custom,
    };
    (class, source, kind)
}
