//! Network providers for built-in web tools.

use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use thiserror::Error;

const BROWSER_USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";
const HONEST_USER_AGENT_VALUE: &str = "BitFun/1.0";
const WEB_FETCH_TIMEOUT_SECS: u64 = 30;
const WEB_FETCH_MAX_TIMEOUT_SECS: u64 = 120;
const WEB_FETCH_MAX_RESPONSE_SIZE: usize = 5 * 1024 * 1024;
const EXA_URL: &str = "https://mcp.exa.ai/mcp";
const EXA_TIMEOUT_SECS: u64 = 25;

#[derive(Debug, Error)]
pub enum WebToolNetworkError {
    #[error("Failed to create HTTP client: {0}")]
    BuildClient(String),
    #[error("Failed to fetch URL: {0}")]
    Fetch(String),
    #[error("HTTP error {status}: {reason}")]
    HttpStatus { status: String, reason: String },
    #[error("Failed to read response: {0}")]
    ReadResponse(String),
    #[error("Failed to send request: {0}")]
    SearchRequest(String),
    #[error("Web search error {status}: {body}")]
    SearchStatus { status: String, body: String },
    #[error("Web search returned no content")]
    SearchEmpty,
}

#[derive(Debug, Clone)]
pub struct WebFetchResponse {
    pub content_type: Option<String>,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ExaSearchRequest<'a> {
    pub query: &'a str,
    pub num_results: u64,
    pub kind: &'a str,
    pub livecrawl: &'a str,
    pub context_max_characters: u64,
}

#[derive(Debug, Deserialize)]
struct ExaResponse {
    result: Option<ExaData>,
}

#[derive(Debug, Deserialize)]
struct ExaData {
    content: Vec<ExaContent>,
}

#[derive(Debug, Deserialize)]
struct ExaContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

pub struct WebToolNetworkProvider;

impl WebToolNetworkProvider {
    fn request(
        client: &reqwest::Client,
        url: &str,
        accept: &str,
        user_agent: &'static str,
    ) -> reqwest::RequestBuilder {
        client
            .get(url)
            .header(reqwest::header::USER_AGENT, user_agent)
            .header(reqwest::header::ACCEPT, accept)
    }

    /// Decode response bytes to a string using the charset declared in the
    /// `Content-Type` header. Falls back to UTF-8 when no charset is
    /// specified, matching the behavior of reqwest's `Response::text()`
    /// for non-UTF-8 encodings (GBK, Shift-JIS, ISO-8859-1, etc.).
    pub fn decode_text_content(content: &[u8], content_type: Option<&str>) -> String {
        let charset = content_type
            .and_then(|ct| {
                ct.split(';').find_map(|part| {
                    let (name, value) = part.split_once('=')?;
                    name.trim()
                        .eq_ignore_ascii_case("charset")
                        .then(|| value.trim().trim_matches('"'))
                })
            })
            .unwrap_or("utf-8");

        let encoding =
            encoding_rs::Encoding::for_label(charset.as_bytes()).unwrap_or(encoding_rs::UTF_8);

        let (decoded, _, _) = encoding.decode(content);
        decoded.into_owned()
    }

    pub async fn fetch(
        url: &str,
        accept: &str,
        timeout_secs: Option<u64>,
    ) -> Result<WebFetchResponse, WebToolNetworkError> {
        let timeout_secs = timeout_secs
            .unwrap_or(WEB_FETCH_TIMEOUT_SECS)
            .min(WEB_FETCH_MAX_TIMEOUT_SECS);
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|error| WebToolNetworkError::BuildClient(error.to_string()))?;

        // Use the honest identifier by default so servers can identify this
        // as a non-browser automation client. Only fall back to a browser UA
        // when a Cloudflare challenge is encountered.
        let mut response = Self::request(&client, url, accept, HONEST_USER_AGENT_VALUE)
            .send()
            .await
            .map_err(|error| WebToolNetworkError::Fetch(error.to_string()))?;

        if response.status() == reqwest::StatusCode::FORBIDDEN
            && response
                .headers()
                .get("cf-mitigated")
                .and_then(|v| v.to_str().ok())
                == Some("challenge")
        {
            response = Self::request(&client, url, accept, BROWSER_USER_AGENT_VALUE)
                .send()
                .await
                .map_err(|error| WebToolNetworkError::Fetch(error.to_string()))?;
        }

        if !response.status().is_success() {
            return Err(WebToolNetworkError::HttpStatus {
                status: response.status().to_string(),
                reason: response
                    .status()
                    .canonical_reason()
                    .unwrap_or("Unknown error")
                    .to_string(),
            });
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        if response
            .content_length()
            .is_some_and(|length| length > WEB_FETCH_MAX_RESPONSE_SIZE as u64)
        {
            return Err(WebToolNetworkError::ReadResponse(
                "Response too large (exceeds 5MB limit)".to_string(),
            ));
        }

        // Stream the body in chunks, aborting as soon as the accumulated
        // size exceeds the limit. This protects against chunked-transfer
        // responses that lack a Content-Length header.
        let mut content = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| WebToolNetworkError::ReadResponse(error.to_string()))?
        {
            content.extend_from_slice(&chunk);
            if content.len() > WEB_FETCH_MAX_RESPONSE_SIZE {
                return Err(WebToolNetworkError::ReadResponse(
                    "Response too large (exceeds 5MB limit)".to_string(),
                ));
            }
        }

        Ok(WebFetchResponse {
            content_type,
            content,
        })
    }

    pub async fn search_exa(request: ExaSearchRequest<'_>) -> Result<String, WebToolNetworkError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(EXA_TIMEOUT_SECS))
            .build()
            .map_err(|error| WebToolNetworkError::BuildClient(error.to_string()))?;

        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "web_search_exa",
                "arguments": {
                    "query": request.query,
                    "type": request.kind,
                    "numResults": request.num_results,
                    "livecrawl": request.livecrawl,
                    "contextMaxCharacters": request.context_max_characters,
                }
            }
        });

        let response = client
            .post(EXA_URL)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|error| WebToolNetworkError::SearchRequest(error.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("Unknown error"));
            return Err(WebToolNetworkError::SearchStatus {
                status: status.to_string(),
                body,
            });
        }

        let text = response
            .text()
            .await
            .map_err(|error| WebToolNetworkError::ReadResponse(error.to_string()))?;

        parse_exa_sse(&text)
    }
}

fn parse_exa_sse(text: &str) -> Result<String, WebToolNetworkError> {
    let out = text
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .find_map(|line| {
            serde_json::from_str::<ExaResponse>(line)
                .ok()
                .and_then(|response| response.result)
                .map(|result| {
                    result
                        .content
                        .into_iter()
                        .filter(|item| item.kind == "text")
                        .filter_map(|item| item.text)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .filter(|item| !item.trim().is_empty())
        });

    out.ok_or(WebToolNetworkError::SearchEmpty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_exa_sse_returns_first_text_payload() {
        let text = concat!(
            "event: message\n",
            "data: {\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"Title: A\\nURL: https://example.com\"}]}}\n",
            "\n"
        );

        let out = parse_exa_sse(text).expect("exa text should parse");

        assert_eq!(out, "Title: A\nURL: https://example.com");
    }

    #[test]
    fn parse_exa_sse_rejects_empty_text_payload() {
        let text = "data: {\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"   \"}]}}\n";

        let error = parse_exa_sse(text).unwrap_err();

        assert!(matches!(error, WebToolNetworkError::SearchEmpty));
    }

    #[test]
    fn decode_text_content_decodes_gbk() {
        // GBK encoded "你好" (hello in Chinese)
        let gbk_bytes: &[u8] = &[0xC4, 0xE3, 0xBA, 0xC3];
        let result =
            WebToolNetworkProvider::decode_text_content(gbk_bytes, Some("text/html; charset=gbk"));
        assert_eq!(result, "你好");
    }

    #[test]
    fn decode_text_content_decodes_shift_jis() {
        // Shift-JIS encoded "日本語" (Japanese)
        let sjis_bytes: &[u8] = &[0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA];
        let result = WebToolNetworkProvider::decode_text_content(
            sjis_bytes,
            Some("text/html; charset=shift_jis"),
        );
        assert_eq!(result, "日本語");
    }

    #[test]
    fn decode_text_content_falls_back_to_utf8() {
        let utf8_bytes = "Hello, world!".as_bytes();
        let result = WebToolNetworkProvider::decode_text_content(utf8_bytes, Some("text/plain"));
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn decode_text_content_handles_quoted_charset() {
        let utf8_bytes = "test".as_bytes();
        let result = WebToolNetworkProvider::decode_text_content(
            utf8_bytes,
            Some("text/html; charset=\"utf-8\""),
        );
        assert_eq!(result, "test");
    }

    #[test]
    fn decode_text_content_handles_case_insensitive_charset_parameter() {
        let gbk_bytes: &[u8] = &[0xC4, 0xE3, 0xBA, 0xC3];
        let result = WebToolNetworkProvider::decode_text_content(
            gbk_bytes,
            Some("text/html; Charset = \"gbk\""),
        );
        assert_eq!(result, "你好");
    }

    #[test]
    fn web_fetch_requests_do_not_force_a_language() {
        let client = reqwest::Client::new();
        for user_agent in [HONEST_USER_AGENT_VALUE, BROWSER_USER_AGENT_VALUE] {
            let request = WebToolNetworkProvider::request(
                &client,
                "https://example.com",
                "text/html",
                user_agent,
            )
            .build()
            .expect("request should build");

            assert!(!request
                .headers()
                .contains_key(reqwest::header::ACCEPT_LANGUAGE));
        }
    }
}
