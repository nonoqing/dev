//! Stateless, privacy-safe projection of authoritative Agent terminal events.
//!
//! This subscriber never stores business identifiers or reconstructs Trace
//! lifecycle from start/end events. Trace spans are owned by the real async
//! execution boundaries. Events contribute only Metric and structured Log
//! records from terminal facts already carried by the event owner.

use crate::event_bus::EventSubscriberResult;
use crate::event_router::EventSubscriber;
use bitfun_core_types::errors::ErrorCategory;
use bitfun_events::{
    AgenticEvent, SafeCountBucket, SafeOperationErrorType, SafeOperationOutcome,
    SafePermissionDecision, SafePermissionSource, SafeSessionClass, SafeSessionOperation,
    ToolEventData, ToolTelemetryKind, ToolTelemetrySourceClass,
};
use bitfun_observability::domains::{
    attempt_bucket, record_compression_terminal, record_inference_terminal, record_inference_usage,
    record_permission_confirmation_terminal, record_permission_evaluation_terminal,
    record_round_terminal, record_session_terminal, record_tool_terminal, record_turn_terminal,
    CompletionFacts, CompressionFinishFacts, CompressionSource, CompressionStartFacts,
    CompressionTrigger, CountBucket, ExitStatusClass, FinishReasonClass, InferenceFinishFacts,
    InferenceStartFacts, InferenceUsageFacts, ModelClass, PermissionConfirmationStartFacts,
    PermissionDecision, PermissionEvaluateStartFacts, PermissionFinishFacts, PermissionSource,
    ProviderClass, RoundFinishFacts, SafeErrorType, SessionClass, SessionFinishFacts,
    SessionOperation, SessionStartFacts, StatusClass, ToolClass, ToolFailureSource,
    ToolFinishFacts, ToolIdentityFacts, ToolKind, ToolSourceClass, TurnFinishFacts,
};
use bitfun_observability::Telemetry;

pub struct AgentTelemetrySubscriber {
    telemetry: Telemetry,
}

impl AgentTelemetrySubscriber {
    pub fn new(telemetry: Telemetry) -> Self {
        Self { telemetry }
    }

    fn project(&self, event: &AgenticEvent) {
        if !self.telemetry.is_enabled() {
            return;
        }
        match event {
            AgenticEvent::DialogTurnCompleted {
                total_rounds,
                total_tools,
                duration_ms,
                success,
                finish_reason,
                partial_recovery_reason,
                first_result_ms,
                modified_file_count,
                added_lines,
                deleted_lines,
                ..
            } => {
                let completion = if success == &Some(false) {
                    CompletionFacts::failed(SafeErrorType::Other)
                } else if partial_recovery_reason.is_some() {
                    CompletionFacts::degraded(SafeErrorType::Provider)
                } else {
                    CompletionFacts::completed()
                };
                record_turn_terminal(
                    &self.telemetry,
                    TurnFinishFacts {
                        completion,
                        finish_reason: finish_reason.as_deref().map(finish_reason_class),
                        round_count: Some(*total_rounds as u64),
                        tool_count: Some(*total_tools as u64),
                        first_result_ms: *first_result_ms,
                        modified_file_count: *modified_file_count,
                        added_lines: *added_lines,
                        deleted_lines: *deleted_lines,
                    },
                    Some(*duration_ms),
                );
            }
            AgenticEvent::DialogTurnCancelled { .. } => record_turn_terminal(
                &self.telemetry,
                TurnFinishFacts {
                    completion: CompletionFacts::cancelled(),
                    finish_reason: Some(FinishReasonClass::Cancelled),
                    round_count: None,
                    tool_count: None,
                    first_result_ms: None,
                    modified_file_count: None,
                    added_lines: None,
                    deleted_lines: None,
                },
                None,
            ),
            AgenticEvent::DialogTurnFailed { error_category, .. } => record_turn_terminal(
                &self.telemetry,
                TurnFinishFacts {
                    completion: CompletionFacts::failed(
                        error_category
                            .as_ref()
                            .map(safe_error_category)
                            .unwrap_or(SafeErrorType::Other),
                    ),
                    finish_reason: Some(FinishReasonClass::Error),
                    round_count: None,
                    tool_count: None,
                    first_result_ms: None,
                    modified_file_count: None,
                    added_lines: None,
                    deleted_lines: None,
                },
                None,
            ),
            AgenticEvent::ModelRoundCompleted {
                has_tool_calls,
                duration_ms,
                provider_id,
                effective_model_name,
                first_chunk_ms,
                first_visible_output_ms,
                attempt_count,
                failure_category,
                ..
            } => {
                // `failure_category` is an untyped display string in the
                // current event contract. Its presence may determine failure,
                // but its content is never classified or exported.
                let completion = if failure_category.is_some() {
                    CompletionFacts::failed(SafeErrorType::Other)
                } else {
                    CompletionFacts::completed()
                };
                let attempts = attempt_count.unwrap_or(1);
                record_round_terminal(
                    &self.telemetry,
                    RoundFinishFacts {
                        completion,
                        has_tool_calls: *has_tool_calls,
                        attempt_bucket: attempt_bucket(attempts),
                    },
                    *duration_ms,
                );
                record_inference_terminal(
                    &self.telemetry,
                    Some(InferenceStartFacts {
                        provider_class: provider_class(
                            provider_id.as_deref(),
                            effective_model_name,
                        ),
                        model_class: model_class(effective_model_name),
                        protocol_class:
                            bitfun_observability::domains::InferenceProtocolClass::Other,
                    }),
                    InferenceFinishFacts {
                        completion,
                        attempt_bucket: attempt_bucket(attempts),
                        status_class: if failure_category.is_some() {
                            None
                        } else {
                            Some(StatusClass::Success)
                        },
                        retryable: None,
                        ttft_ms: first_visible_output_ms.or(*first_chunk_ms),
                        input_tokens: None,
                        output_tokens: None,
                        reasoning_tokens: None,
                        cache_read_tokens: None,
                    },
                    *duration_ms,
                );
            }
            AgenticEvent::TokenUsageUpdated {
                effective_model_name,
                input_tokens,
                output_tokens,
                is_subagent,
                cached_tokens,
                reasoning_tokens,
                ..
            } => record_inference_usage(
                &self.telemetry,
                InferenceUsageFacts {
                    provider_class: provider_class(None, effective_model_name),
                    model_class: model_class(effective_model_name),
                    subagent: *is_subagent,
                    input_tokens: *input_tokens as u64,
                    output_tokens: output_tokens.map(|tokens| tokens as u64),
                    reasoning_tokens: reasoning_tokens.map(|tokens| tokens as u64),
                    cache_read_tokens: cached_tokens.map(|tokens| tokens as u64),
                },
            ),
            AgenticEvent::ContextCompressionCompleted {
                trigger,
                tokens_before,
                tokens_after,
                duration_ms,
                has_summary,
                summary_source,
                ..
            } => record_compression_terminal(
                &self.telemetry,
                CompressionStartFacts {
                    trigger: compression_trigger(trigger),
                },
                CompressionFinishFacts {
                    completion: CompletionFacts::completed(),
                    source: Some(compression_source(summary_source)),
                    has_summary: Some(*has_summary),
                    tokens_before: Some(*tokens_before as u64),
                    tokens_after: Some(*tokens_after as u64),
                },
                Some(*duration_ms),
            ),
            AgenticEvent::ContextCompressionFailed {
                trigger,
                duration_ms,
                tokens_before,
                ..
            } => record_compression_terminal(
                &self.telemetry,
                CompressionStartFacts {
                    trigger: compression_trigger(trigger),
                },
                CompressionFinishFacts {
                    completion: CompletionFacts::failed(SafeErrorType::Internal),
                    source: None,
                    has_summary: None,
                    tokens_before: tokens_before.map(|tokens| tokens as u64),
                    tokens_after: None,
                },
                Some(*duration_ms),
            ),
            AgenticEvent::SessionOperationCompleted {
                operation,
                session_class,
                remote,
                outcome,
                error_type,
                duration_ms,
            } => record_session_terminal(
                &self.telemetry,
                SessionStartFacts {
                    operation: session_operation(*operation),
                    session_class: telemetry_session_class(*session_class),
                    remote: *remote,
                },
                SessionFinishFacts {
                    completion: terminal_completion(*outcome, *error_type),
                },
                Some(*duration_ms),
            ),
            AgenticEvent::PermissionEvaluationCompleted {
                intent_count_bucket,
                delegated,
                decision,
                source,
                outcome,
                error_type,
                duration_ms,
            } => record_permission_evaluation_terminal(
                &self.telemetry,
                PermissionEvaluateStartFacts {
                    intent_count_bucket: count_bucket_from_event(*intent_count_bucket),
                    delegated: *delegated,
                },
                PermissionFinishFacts {
                    completion: terminal_completion(*outcome, *error_type),
                    decision: permission_decision(*decision),
                    source: permission_source(*source),
                },
                Some(*duration_ms),
            ),
            AgenticEvent::PermissionConfirmationCompleted {
                request_count_bucket,
                auto_approve,
                decision,
                source,
                outcome,
                error_type,
                duration_ms,
            } => record_permission_confirmation_terminal(
                &self.telemetry,
                PermissionConfirmationStartFacts {
                    request_count_bucket: count_bucket_from_event(*request_count_bucket),
                    auto_approve: *auto_approve,
                },
                PermissionFinishFacts {
                    completion: terminal_completion(*outcome, *error_type),
                    decision: permission_decision(*decision),
                    source: permission_source(*source),
                },
                Some(*duration_ms),
            ),
            AgenticEvent::ToolEvent { tool_event, .. } => self.project_tool_terminal(tool_event),
            _ => {}
        }
    }

    fn project_tool_terminal(&self, event: &ToolEventData) {
        let telemetry_identity = event.identity().telemetry.as_ref();
        let source_class = telemetry_identity
            .map(|identity| tool_source_class(&identity.source_class))
            .unwrap_or(ToolSourceClass::Custom);
        let identity = ToolIdentityFacts {
            tool_class: match source_class {
                ToolSourceClass::BuiltIn | ToolSourceClass::Skill => ToolClass::BuiltIn,
                _ => ToolClass::Custom,
            },
            source_class,
            tool_kind: telemetry_identity
                .map(|identity| telemetry_tool_kind(&identity.kind))
                .unwrap_or(ToolKind::Other),
            parallel: telemetry_identity.is_some_and(|identity| identity.parallel),
            remote: telemetry_identity.is_some_and(|identity| identity.remote),
            background: telemetry_identity.is_some_and(|identity| identity.background),
        };
        match event {
            ToolEventData::Completed {
                duration_ms,
                queue_wait_ms,
                preflight_ms,
                confirmation_wait_ms,
                execution_ms,
                ..
            } => record_tool_terminal(
                &self.telemetry,
                Some(identity),
                ToolFinishFacts {
                    completion: CompletionFacts::completed(),
                    queue_ms: *queue_wait_ms,
                    preflight_ms: *preflight_ms,
                    confirmation_ms: *confirmation_wait_ms,
                    execution_ms: *execution_ms,
                    failure_source: None,
                    exit_status_class: Some(ExitStatusClass::Success),
                },
                Some(*duration_ms),
            ),
            ToolEventData::Failed {
                duration_ms,
                queue_wait_ms,
                preflight_ms,
                confirmation_wait_ms,
                execution_ms,
                ..
            } => record_tool_terminal(
                &self.telemetry,
                Some(identity),
                ToolFinishFacts {
                    // The event carries only raw error text, which is ignored.
                    completion: CompletionFacts::failed(SafeErrorType::Other),
                    queue_ms: *queue_wait_ms,
                    preflight_ms: *preflight_ms,
                    confirmation_ms: *confirmation_wait_ms,
                    execution_ms: *execution_ms,
                    failure_source: Some(ToolFailureSource::Execution),
                    exit_status_class: Some(ExitStatusClass::Unknown),
                },
                *duration_ms,
            ),
            ToolEventData::Cancelled {
                duration_ms,
                queue_wait_ms,
                preflight_ms,
                confirmation_wait_ms,
                execution_ms,
                ..
            } => record_tool_terminal(
                &self.telemetry,
                Some(identity),
                ToolFinishFacts {
                    completion: CompletionFacts::cancelled(),
                    queue_ms: *queue_wait_ms,
                    preflight_ms: *preflight_ms,
                    confirmation_ms: *confirmation_wait_ms,
                    execution_ms: *execution_ms,
                    failure_source: Some(ToolFailureSource::Cancellation),
                    exit_status_class: None,
                },
                *duration_ms,
            ),
            ToolEventData::Rejected { .. } => record_tool_terminal(
                &self.telemetry,
                Some(identity),
                ToolFinishFacts {
                    completion: CompletionFacts::rejected(SafeErrorType::PermissionDenied),
                    queue_ms: None,
                    preflight_ms: None,
                    confirmation_ms: None,
                    execution_ms: None,
                    failure_source: Some(ToolFailureSource::Permission),
                    exit_status_class: None,
                },
                None,
            ),
            _ => {}
        }
    }
}

#[async_trait::async_trait]
impl EventSubscriber for AgentTelemetrySubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> EventSubscriberResult {
        self.project(event);
        Ok(())
    }
}

fn terminal_completion(
    outcome: SafeOperationOutcome,
    error_type: Option<SafeOperationErrorType>,
) -> CompletionFacts {
    let error_type = error_type.map(safe_operation_error_type);
    match outcome {
        SafeOperationOutcome::Completed => CompletionFacts::completed(),
        SafeOperationOutcome::Failed => {
            CompletionFacts::failed(error_type.unwrap_or(SafeErrorType::Other))
        }
        SafeOperationOutcome::Cancelled => CompletionFacts::cancelled(),
        SafeOperationOutcome::Timeout => CompletionFacts::timeout(),
        SafeOperationOutcome::Rejected => {
            CompletionFacts::rejected(error_type.unwrap_or(SafeErrorType::Other))
        }
        SafeOperationOutcome::Degraded => {
            CompletionFacts::degraded(error_type.unwrap_or(SafeErrorType::Other))
        }
        SafeOperationOutcome::Incomplete => CompletionFacts::incomplete(),
    }
}

fn safe_operation_error_type(value: SafeOperationErrorType) -> SafeErrorType {
    match value {
        SafeOperationErrorType::Cancelled => SafeErrorType::Cancelled,
        SafeOperationErrorType::Timeout => SafeErrorType::Timeout,
        SafeOperationErrorType::Authentication => SafeErrorType::Authentication,
        SafeOperationErrorType::RateLimited => SafeErrorType::RateLimited,
        SafeOperationErrorType::NetworkUnavailable => SafeErrorType::NetworkUnavailable,
        SafeOperationErrorType::NetworkProtocol => SafeErrorType::NetworkProtocol,
        SafeOperationErrorType::InvalidRequest => SafeErrorType::InvalidRequest,
        SafeOperationErrorType::ContextOverflow => SafeErrorType::ContextOverflow,
        SafeOperationErrorType::ToolValidation => SafeErrorType::ToolValidation,
        SafeOperationErrorType::PermissionDenied => SafeErrorType::PermissionDenied,
        SafeOperationErrorType::Persistence => SafeErrorType::Persistence,
        SafeOperationErrorType::Provider => SafeErrorType::Provider,
        SafeOperationErrorType::Internal => SafeErrorType::Internal,
        SafeOperationErrorType::Other => SafeErrorType::Other,
    }
}

fn session_operation(value: SafeSessionOperation) -> SessionOperation {
    match value {
        SafeSessionOperation::Create => SessionOperation::Create,
        SafeSessionOperation::Resume => SessionOperation::Resume,
        SafeSessionOperation::Delete => SessionOperation::Delete,
    }
}

fn telemetry_session_class(value: SafeSessionClass) -> SessionClass {
    match value {
        SafeSessionClass::Standard => SessionClass::Standard,
        SafeSessionClass::Subagent => SessionClass::Subagent,
        SafeSessionClass::Internal => SessionClass::Internal,
        SafeSessionClass::Transient => SessionClass::Transient,
    }
}

fn count_bucket_from_event(value: SafeCountBucket) -> CountBucket {
    match value {
        SafeCountBucket::Zero => CountBucket::Zero,
        SafeCountBucket::One => CountBucket::One,
        SafeCountBucket::Two => CountBucket::Two,
        SafeCountBucket::ThreePlus => CountBucket::ThreePlus,
    }
}

fn permission_decision(value: SafePermissionDecision) -> PermissionDecision {
    match value {
        SafePermissionDecision::Allow => PermissionDecision::Allow,
        SafePermissionDecision::Ask => PermissionDecision::Ask,
        SafePermissionDecision::PolicyDeny => PermissionDecision::PolicyDeny,
        SafePermissionDecision::UserReject => PermissionDecision::UserReject,
        SafePermissionDecision::Cancelled => PermissionDecision::Cancelled,
        SafePermissionDecision::Failed => PermissionDecision::Failed,
    }
}

fn permission_source(value: SafePermissionSource) -> PermissionSource {
    match value {
        SafePermissionSource::Policy => PermissionSource::Policy,
        SafePermissionSource::StoredGrant => PermissionSource::StoredGrant,
        SafePermissionSource::Hook => PermissionSource::Hook,
        SafePermissionSource::AutoApprove => PermissionSource::AutoApprove,
        SafePermissionSource::User => PermissionSource::User,
        SafePermissionSource::Delegated => PermissionSource::Delegated,
        SafePermissionSource::Other => PermissionSource::Other,
    }
}

fn safe_error_category(category: &ErrorCategory) -> SafeErrorType {
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

fn finish_reason_class(value: &str) -> FinishReasonClass {
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

fn compression_trigger(value: &str) -> CompressionTrigger {
    match value {
        "auto" | "threshold" => CompressionTrigger::Threshold,
        "context_overflow" | "context_overflow_recovery" => CompressionTrigger::ContextOverflow,
        "manual" | "compact" => CompressionTrigger::Manual,
        "recovery" => CompressionTrigger::Recovery,
        _ => CompressionTrigger::Other,
    }
}

fn compression_source(value: &str) -> CompressionSource {
    match value {
        "model" => CompressionSource::Model,
        "local_fallback" => CompressionSource::LocalFallback,
        _ => CompressionSource::None,
    }
}

fn provider_class(provider: Option<&str>, model: &str) -> ProviderClass {
    let value = provider.unwrap_or(model).to_ascii_lowercase();
    if value.contains("anthropic") || value.contains("claude") {
        ProviderClass::AnthropicCompatible
    } else if value.contains("google") || value.contains("gemini") {
        ProviderClass::GoogleCompatible
    } else if value.contains("openai") || value.contains("gpt") || value.contains("codex") {
        ProviderClass::OpenAiCompatible
    } else if value.contains("ollama") || value.contains("local") {
        ProviderClass::Local
    } else {
        ProviderClass::Other
    }
}

fn model_class(model: &str) -> ModelClass {
    let model = model.to_ascii_lowercase();
    if model.contains("vision") || model.contains("vl") {
        ModelClass::Vision
    } else if model.contains("code") || model.contains("codex") {
        ModelClass::Code
    } else if model.contains("flash") || model.contains("mini") || model.contains("haiku") {
        ModelClass::Fast
    } else if model.is_empty() {
        ModelClass::Other
    } else {
        ModelClass::GeneralReasoning
    }
}

fn tool_source_class(source: &ToolTelemetrySourceClass) -> ToolSourceClass {
    match source {
        ToolTelemetrySourceClass::BuiltIn => ToolSourceClass::BuiltIn,
        ToolTelemetrySourceClass::Mcp => ToolSourceClass::Mcp,
        ToolTelemetrySourceClass::Skill => ToolSourceClass::Skill,
        ToolTelemetrySourceClass::Plugin => ToolSourceClass::Plugin,
        ToolTelemetrySourceClass::External => ToolSourceClass::External,
        ToolTelemetrySourceClass::Custom => ToolSourceClass::Custom,
    }
}

fn telemetry_tool_kind(kind: &ToolTelemetryKind) -> ToolKind {
    match kind {
        ToolTelemetryKind::Filesystem => ToolKind::Filesystem,
        ToolTelemetryKind::Search => ToolKind::Search,
        ToolTelemetryKind::Shell => ToolKind::Shell,
        ToolTelemetryKind::Git => ToolKind::Git,
        ToolTelemetryKind::Browser => ToolKind::Browser,
        ToolTelemetryKind::ComputerUse => ToolKind::ComputerUse,
        ToolTelemetryKind::Protocol => ToolKind::Protocol,
        ToolTelemetryKind::Task => ToolKind::Task,
        ToolTelemetryKind::Other => ToolKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_events::{
        ToolEventIdentity, ToolTelemetryIdentity, ToolTelemetryKind, ToolTelemetrySourceClass,
    };
    use bitfun_observability::{InMemorySink, PolicySnapshot, TelemetryLevel, ValidatedRecord};
    use std::sync::Arc;

    #[tokio::test]
    async fn terminal_projection_drops_ids_content_and_raw_errors() {
        let sink = Arc::new(InMemorySink::default());
        let (telemetry, _) = Telemetry::build(
            PolicySnapshot::new(TelemetryLevel::Basic).with_success_log_sample_ratio(1.0),
            sink.clone(),
        );
        let subscriber = AgentTelemetrySubscriber::new(telemetry);
        let secret = "SENSITIVE_PROMPT_/Users/alice/project_API_RESPONSE";
        subscriber
            .on_event(&AgenticEvent::ToolEvent {
                session_id: secret.to_string(),
                turn_id: secret.to_string(),
                round_id: secret.to_string(),
                attempt_id: Some(secret.to_string()),
                attempt_index: Some(1),
                tool_event: ToolEventData::Failed {
                    identity: ToolEventIdentity::direct(secret, secret).with_telemetry(
                        ToolTelemetryIdentity {
                            source_class: ToolTelemetrySourceClass::Mcp,
                            kind: ToolTelemetryKind::Protocol,
                            parallel: true,
                            remote: false,
                            background: false,
                        },
                    ),
                    error: secret.to_string(),
                    duration_ms: Some(42),
                    queue_wait_ms: Some(1),
                    preflight_ms: Some(2),
                    confirmation_wait_ms: None,
                    execution_ms: Some(39),
                },
            })
            .await
            .unwrap();

        let records = sink.records();
        assert!(!records.is_empty());
        assert!(records
            .iter()
            .all(|record| !matches!(record, ValidatedRecord::Span(_))));
        let encoded = serde_json::to_string(&records).unwrap();
        assert!(!encoded.contains(secret));
        assert!(!encoded.contains("session_id"));
        assert!(!encoded.contains("tool_id"));
        assert!(encoded.contains("mcp"));
        assert!(encoded.contains("protocol"));
    }

    #[tokio::test]
    async fn compression_terminal_event_projects_once_without_raw_error() {
        let sink = Arc::new(InMemorySink::default());
        let (telemetry, _) = Telemetry::build(
            PolicySnapshot::new(TelemetryLevel::Basic).with_success_log_sample_ratio(1.0),
            sink.clone(),
        );
        let subscriber = AgentTelemetrySubscriber::new(telemetry);
        let secret = "SENSITIVE_/Users/alice/private_prompt";
        subscriber
            .on_event(&AgenticEvent::ContextCompressionFailed {
                session_id: secret.to_string(),
                turn_id: secret.to_string(),
                compression_id: secret.to_string(),
                trigger: "context_overflow_recovery".to_string(),
                duration_ms: 42,
                tokens_before: Some(100),
                error: secret.to_string(),
            })
            .await
            .unwrap();

        let records = sink.records();
        assert_eq!(records.len(), 3);
        let encoded = serde_json::to_string(&records).unwrap();
        assert!(!encoded.contains(secret));
        assert!(encoded.contains("context_overflow"));
        assert!(encoded.contains("internal"));
    }

    #[tokio::test]
    async fn safe_session_and_permission_events_project_without_trace_or_identifiers() {
        let sink = Arc::new(InMemorySink::default());
        let (telemetry, _) = Telemetry::build(
            PolicySnapshot::new(TelemetryLevel::Basic).with_success_log_sample_ratio(1.0),
            sink.clone(),
        );
        let subscriber = AgentTelemetrySubscriber::new(telemetry);
        for event in [
            AgenticEvent::SessionOperationCompleted {
                operation: SafeSessionOperation::Resume,
                session_class: SafeSessionClass::Standard,
                remote: false,
                outcome: SafeOperationOutcome::Completed,
                error_type: None,
                duration_ms: 11,
            },
            AgenticEvent::PermissionEvaluationCompleted {
                intent_count_bucket: SafeCountBucket::Two,
                delegated: false,
                decision: SafePermissionDecision::Ask,
                source: SafePermissionSource::Policy,
                outcome: SafeOperationOutcome::Completed,
                error_type: None,
                duration_ms: 12,
            },
            AgenticEvent::PermissionConfirmationCompleted {
                request_count_bucket: SafeCountBucket::One,
                auto_approve: false,
                decision: SafePermissionDecision::UserReject,
                source: SafePermissionSource::User,
                outcome: SafeOperationOutcome::Rejected,
                error_type: Some(SafeOperationErrorType::PermissionDenied),
                duration_ms: 13,
            },
        ] {
            subscriber.on_event(&event).await.unwrap();
        }

        let records = sink.records();
        assert_eq!(records.len(), 9);
        assert!(records
            .iter()
            .all(|record| !matches!(record, ValidatedRecord::Span(_))));
        let encoded = serde_json::to_string(&records).unwrap();
        assert!(!encoded.contains("session_id"));
        assert!(!encoded.contains("request_id"));
        assert!(!encoded.contains("tool_id"));
        assert!(encoded.contains("bitfun.agent.session"));
        assert!(encoded.contains("bitfun.permission.evaluate"));
        assert!(encoded.contains("bitfun.permission.confirmation"));
    }

    #[test]
    fn error_mapping_uses_typed_category_only() {
        assert_eq!(
            safe_error_category(&ErrorCategory::RateLimit),
            SafeErrorType::RateLimited
        );
        assert_eq!(
            safe_error_category(&ErrorCategory::Unknown),
            SafeErrorType::Other
        );
    }
}
