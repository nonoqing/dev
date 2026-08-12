use super::super::util::{
    primary_api_format, require_multimodal_tool_output, supported_image_mime_type,
};
use super::readable::{
    extract_html_title, extract_markdown_with_text_fallback, is_html, normalize_requested_format,
    RequestedFormat,
};
use crate::agentic::image_analysis::optimize_image_for_provider;
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolExposure, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::ToolImageAttachment;
use async_trait::async_trait;
use base64::Engine as _;
use bitfun_services_integrations::web_tools::WebToolNetworkProvider;
use serde_json::{json, Value};

/// WebFetch tool
pub struct WebFetchTool;

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "WebFetch"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Fetch content from a URL.

Use this tool to:
- Read documentation from websites
- Fetch API responses
- Download readable content from web pages
- Access online resources

Best for static pages that need no login. For pages requiring the user's login session or JavaScript rendering, use ControlHub domain="browser" instead: connect -> navigate -> snapshot / read_article. Chrome 144+ and Edge can connect to the user's current profile after explicit approval, preserving tabs and login state; other supported Chromium browsers reuse a real-profile endpoint when available and otherwise use BitFun's persistent managed profile. (browser.fetch only works when a session is already connected and the current page is same-origin with the target URL — it runs inside that page and is subject to its CORS policy.)

Supports different output formats:
- raw/html: Raw response content (original HTML or text)
- text: Plain text (HTML tags, scripts, and styles removed)
- markdown: Readable content mode. For HTML pages, BitFun extracts the main content and returns markdown when possible, automatically falling back to plain text when markdown conversion is not reliable.
- json: Parse JSON responses

Image responses (PNG, JPEG, GIF, WebP, etc.) are automatically detected and returned as image attachments, regardless of the requested format.

An optional timeout (1-120 seconds, default 30) can be specified to control how long the request may take.

Example usage:
- Fetch raw HTML: {"url": "https://example.com", "format": "raw"}
- Fetch plain text: {"url": "https://example.com", "format": "text"}
- Fetch readable content: {"url": "https://example.com/article", "format": "markdown"}
- Get API data: {"url": "https://api.example.com/data", "format": "json"}
- Fetch with timeout: {"url": "https://example.com", "format": "markdown", "timeout": 60}"#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Fetch content from a URL in raw/html, text, markdown, or JSON format.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "format": {
                    "type": "string",
                    "enum": ["raw", "html", "text", "markdown", "json"],
                    "description": "Output format: raw/html, text, markdown, or json.",
                    "default": "markdown"
                },
                "timeout": {
                    "type": "number",
                    "minimum": 1,
                    "maximum": 120,
                    "description": "Optional timeout in seconds (1-120)."
                }
            },
            "required": ["url"]
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        let url = input
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .ok_or_else(|| BitFunError::validation("url is required".to_string()))?;
        Ok(vec![PermissionIntent::new(
            "webfetch",
            vec![url.to_string()],
        )])
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
            if url.is_empty() {
                return ValidationResult {
                    result: false,
                    message: Some("URL cannot be empty".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }

            if !url.starts_with("http://") && !url.starts_with("https://") {
                return ValidationResult {
                    result: false,
                    message: Some("URL must start with http:// or https://".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        } else {
            return ValidationResult {
                result: false,
                message: Some("url is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if let Some(timeout) = input.get("timeout") {
            let Some(timeout) = timeout.as_f64() else {
                return ValidationResult {
                    result: false,
                    message: Some("timeout must be a number".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            };
            if !timeout.is_finite() || !(1.0..=120.0).contains(&timeout) {
                return ValidationResult {
                    result: false,
                    message: Some("timeout must be between 1 and 120 seconds".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("url is required".to_string()))?;

        let requested_format =
            normalize_requested_format(input.get("format").and_then(|v| v.as_str()))?;

        let timeout = input
            .get("timeout")
            .and_then(Value::as_f64)
            .map(|v| v.ceil() as u64);
        let accept = match requested_format {
            RequestedFormat::Markdown => "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1",
            RequestedFormat::Text => "text/plain;q=1.0, text/markdown;q=0.9, text/html;q=0.8, */*;q=0.1",
            RequestedFormat::Raw => "text/html;q=1.0, application/xhtml+xml;q=0.9, text/plain;q=0.8, text/markdown;q=0.7, */*;q=0.1",
            RequestedFormat::Json => "application/json;q=1.0, */*;q=0.1",
        };

        let response = WebToolNetworkProvider::fetch(url, accept, timeout)
            .await
            .map_err(|error| BitFunError::tool(error.to_string()))?;
        let content_type = response.content_type;
        let mime = content_type
            .as_deref()
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if mime.starts_with("image/") && mime != "image/svg+xml" {
            require_multimodal_tool_output("WebFetch", context)?;
            let original_mime_type =
                supported_image_mime_type(&response.content).ok_or_else(|| {
                    BitFunError::tool(
                        "WebFetch can only attach supported image files: png, jpeg, gif, webp, or bmp"
                            .to_string(),
                    )
                })?;
            let provider = primary_api_format(context);
            let processed =
                optimize_image_for_provider(response.content, &provider, Some(original_mime_type))
                    .map_err(|error| {
                        BitFunError::tool(format!(
                            "unable to prepare WebFetch image for model vision: {}",
                            error
                        ))
                    })?;
            let size = processed.data.len();
            let mime_type = processed.mime_type;
            return Ok(vec![ToolResult::ok_with_images(
                json!({"url": url, "mime_type": mime_type, "format": "image", "content_representation": "image", "content_length": size, "content": "Image fetched successfully"}),
                Some("Image fetched successfully".to_string()),
                vec![ToolImageAttachment {
                    mime_type,
                    data_base64: base64::engine::general_purpose::STANDARD.encode(processed.data),
                }],
            )]);
        }
        let content =
            WebToolNetworkProvider::decode_text_content(&response.content, content_type.as_deref());

        let is_html_response = is_html(content_type.as_deref(), &content);
        let fallback_title = if is_html_response {
            extract_html_title(&content)
        } else {
            None
        };

        let (processed_content, content_representation, extractor, title) = match requested_format {
            RequestedFormat::Raw => (content, "raw", "raw", fallback_title),
            RequestedFormat::Json => {
                serde_json::from_str::<Value>(&content)
                    .map_err(|e| BitFunError::tool(format!("Invalid JSON response: {}", e)))?;
                (content, "json", "json", None)
            }
            RequestedFormat::Text => {
                if is_html_response {
                    (
                        super::readable::html_to_text(&content),
                        "plain_text",
                        "html_to_text",
                        fallback_title,
                    )
                } else if mime == "text/plain" {
                    (content, "plain_text", "server", None)
                } else {
                    (content, "plain_text", "plain_text", None)
                }
            }
            RequestedFormat::Markdown => {
                if is_html_response {
                    let readable = extract_markdown_with_text_fallback(&content, url)?;
                    (
                        readable.content,
                        readable.content_representation,
                        readable.extractor,
                        readable.title,
                    )
                } else if mime == "text/markdown" || mime == "text/x-markdown" {
                    (content, "markdown", "server", None)
                } else {
                    (content, "plain_text", "plain_text", None)
                }
            }
        };

        let result = ToolResult::Result {
            data: json!({
                "url": url,
                "title": title,
                "format": match requested_format {
                    RequestedFormat::Raw => "raw",
                    RequestedFormat::Markdown => "markdown",
                    RequestedFormat::Text => "text",
                    RequestedFormat::Json => "json",
                },
                "content_representation": content_representation,
                "extractor": extractor,
                "content": processed_content,
                "content_length": processed_content.len()
            }),
            result_for_assistant: Some(processed_content),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}
