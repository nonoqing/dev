use super::{common, OpenAIMessageConverter};
use crate::client::quirks::should_append_tool_stream;
use crate::client::sse::execute_sse_request;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_openai_stream;
use crate::trace::ModelExchangeTraceConfig;
use crate::types::{GeminiResponse, Message, ToolDefinition};
use anyhow::{anyhow, Result};
use log::{debug, info, warn};

/// Build request body for non-streaming request (to get usage data)
pub(crate) fn build_non_stream_request_body(
    client: &AIClient,
    url: &str,
    openai_messages: Vec<serde_json::Value>,
) -> serde_json::Value {
    let mut request_body = serde_json::json!({
        "model": client.config.model,
        "messages": openai_messages,
        "stream": false
    });

    common::apply_reasoning_fields(&mut request_body, client, url);

    // For non-streaming fallback, use a minimal max_tokens to reduce cost
    // We only need the usage data, not the full response
    if let Some(max_tokens) = client.config.max_tokens {
        request_body["max_tokens"] = serde_json::json!(max_tokens);
    } else {
        // Set a small default to get token counts without generating full response
        request_body["max_tokens"] = serde_json::json!(1);
    }

    request_body
}

/// Send a non-streaming request to get usage data
pub(crate) async fn send_non_stream(
    client: &AIClient,
    messages: Vec<Message>,
) -> Result<GeminiResponse> {
    let url = client.config.request_url.clone();
    info!(
        "Sending non-streaming request for usage data: model={}, url={}",
        client.config.model, url
    );

    let openai_messages = OpenAIMessageConverter::convert_messages(messages);
    let request_body = build_non_stream_request_body(client, &url, openai_messages);

    let response = common::apply_headers(client, client.client.post(&url))
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        warn!(
            "Non-streaming request failed: status={}, body={}",
            status, error_body
        );
        return Err(anyhow!("API request failed: {} - {}", status, error_body));
    }

    let response_json: serde_json::Value = response.json().await?;
    info!(
        "Non-streaming response received: has_usage={}",
        response_json.get("usage").is_some()
    );

    // Parse the response into our unified format
    let usage = parse_usage_from_response(&response_json);
    let choices = response_json
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first());

    let text = choices
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .unwrap_or("")
        .to_string();

    let finish_reason = choices
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(|fr| fr.as_str())
        .map(|s| s.to_string());

    Ok(GeminiResponse {
        text,
        reasoning_content: None,
        tool_calls: None,
        usage,
        finish_reason,
        provider_metadata: None,
    })
}

/// Parse usage data from non-streaming API response
fn parse_usage_from_response(response: &serde_json::Value) -> Option<crate::types::GeminiUsage> {
    let usage = response.get("usage")?;

    let prompt_tokens = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let completion_tokens = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let total_tokens = usage
        .get("total_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(prompt_tokens as u64 + completion_tokens as u64) as u32;

    let reasoning_tokens = usage
        .get("reasoning_tokens")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let cached_tokens = usage
        .get("cached_tokens")
        .or_else(|| usage.get("prompt_cache_hit_tokens"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let creation_tokens = usage
        .get("prompt_cache_miss_tokens")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    Some(crate::types::GeminiUsage {
        prompt_token_count: prompt_tokens,
        candidates_token_count: completion_tokens,
        total_token_count: total_tokens,
        reasoning_token_count: reasoning_tokens,
        cached_content_token_count: cached_tokens,
        cache_creation_token_count: creation_tokens,
    })
}

pub(crate) fn try_build_request_body(
    client: &AIClient,
    url: &str,
    openai_messages: Vec<serde_json::Value>,
    openai_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    let mut request_body = serde_json::json!({
        "model": client.config.model,
        "messages": openai_messages,
        "stream": true,
        "stream_options": {
            "include_usage": true
        }
    });

    let model_name = client.config.model.to_lowercase();

    if should_append_tool_stream(url, &model_name) {
        request_body["tool_stream"] = serde_json::Value::Bool(true);
    }

    let base_reasoning_fields = shared::capture_reasoning_fields(
        &request_body,
        &["thinking", "enable_thinking", "reasoning_effort"],
        &[],
    );

    if let Some(max_tokens) = client.config.max_tokens {
        request_body["max_tokens"] = serde_json::json!(max_tokens);
    }

    let protected_keys = &[
        "model",
        "messages",
        "stream",
        "max_tokens",
        "tool_stream",
        "tools",
    ];
    if let Some(preset) = client.model_reasoning_preset.as_ref() {
        shared::apply_reasoning_actions(
            preset,
            &mut request_body,
            protected_keys,
            &[],
            |action, body| {
                common::compile_chat_reasoning_action(
                    preset,
                    action,
                    body,
                    url,
                    &client.config.model,
                )
            },
        )?;
    }

    let protected_body = shared::protect_request_body(
        client,
        &mut request_body,
        &["model", "messages", "stream", "max_tokens", "tool_stream", "stream_options"],
        &[],
    );

    if let Some(extra) = extra_body {
        if let Some(extra_obj) = extra.as_object() {
            shared::merge_extra_body(&mut request_body, extra_obj);
            shared::log_extra_body_keys("ai::openai_stream_request", extra_obj);
        }
    }

    shared::restore_protected_body(&mut request_body, protected_body);
    if let Some(preset) = client.selected_reasoning_preset.as_ref() {
        shared::reset_reasoning_fields(
            &mut request_body,
            base_reasoning_fields.as_ref(),
            &["thinking", "enable_thinking", "reasoning_effort"],
            &[],
        );
        shared::apply_reasoning_actions(
            preset,
            &mut request_body,
            protected_keys,
            &[],
            |action, body| {
                common::compile_chat_reasoning_action(
                    preset,
                    action,
                    body,
                    url,
                    &client.config.model,
                )
            },
        )?;
    }

    if let Some(request_obj) = request_body.as_object_mut() {
        if let Some(existing_n) = request_obj.remove("n") {
            warn!(
                target: "ai::openai_stream_request",
                "Removed custom request field n={} because the stream processor only handles the first choice",
                existing_n
            );
        }
    }

    shared::log_request_body(
        "ai::openai_stream_request",
        "OpenAI stream request body (excluding tools):",
        &request_body,
    );

    common::attach_tools(&mut request_body, openai_tools, "ai::openai_stream_request");

    Ok(request_body)
}

#[cfg(test)]
pub(crate) fn build_request_body(
    client: &AIClient,
    url: &str,
    openai_messages: Vec<serde_json::Value>,
    openai_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
) -> serde_json::Value {
    try_build_request_body(client, url, openai_messages, openai_tools, extra_body)
        .expect("request body should compile")
}

pub(crate) async fn send_stream(
    client: &AIClient,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    extra_body: Option<serde_json::Value>,
    max_tries: usize,
    trace: Option<ModelExchangeTraceConfig>,
) -> Result<StreamResponse> {
    let url = client.config.request_url.clone();
    debug!(
        "OpenAI config: model={}, request_url={}, max_tries={}",
        client.config.model, client.config.request_url, max_tries
    );

    let openai_messages = OpenAIMessageConverter::convert_messages(messages);
    let openai_tools = OpenAIMessageConverter::convert_tools(tools);
    let request_body =
        try_build_request_body(client, &url, openai_messages, openai_tools, extra_body)?;
    let inline_think_in_text = client.config.inline_think_in_text;
    let idle_timeout = client.stream_options.idle_timeout;
    let ttft_timeout = client.stream_options.ttft_timeout;

    execute_sse_request(
        "OpenAI Streaming API",
        &url,
        &request_body,
        max_tries,
        ttft_timeout,
        trace,
        || common::apply_headers(client, client.client.post(&url)),
        move |response, tx, tx_raw, remaining_ttft_timeout| {
            handle_openai_stream(
                response,
                tx,
                tx_raw,
                inline_think_in_text,
                remaining_ttft_timeout,
                idle_timeout,
            )
        },
    )
    .await
}
