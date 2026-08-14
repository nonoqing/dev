use crate::diagnostics::TransportDiagnostics;
use crate::settings::{TelemetryRetryConfig, ValidatedTelemetrySettings};
use crate::TelemetryRuntimeError;
use opentelemetry_http::{Bytes, HttpClient, HttpError, Request, Response};
use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use opentelemetry_proto::tonic::collector::metrics::v1::{
    ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use prost::Message;
use rand::Rng;
use std::error::Error;
use std::fmt;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub(crate) struct GenerationGate {
    active: AtomicBool,
}

impl GenerationGate {
    pub(crate) fn new() -> Self {
        Self {
            active: AtomicBool::new(true),
        }
    }

    pub(crate) fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

#[derive(Clone)]
pub(crate) struct GuardedHttpClient {
    inner: reqwest::Client,
    configured_headers: Vec<(reqwest::header::HeaderName, reqwest::header::HeaderValue)>,
    gate: Arc<GenerationGate>,
    retry: TelemetryRetryConfig,
    diagnostics: Arc<TransportDiagnostics>,
}

impl fmt::Debug for GuardedHttpClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GuardedHttpClient")
            .field("active", &self.gate.is_active())
            .field("retry", &self.retry)
            .finish_non_exhaustive()
    }
}

impl GuardedHttpClient {
    pub(crate) fn new(
        settings: &ValidatedTelemetrySettings,
        gate: Arc<GenerationGate>,
        diagnostics: Arc<TransportDiagnostics>,
    ) -> Result<Self, TelemetryRuntimeError> {
        let timeout = settings.request_timeout();
        let inner = reqwest::Client::builder()
            .use_rustls_tls()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .build()
            .map_err(|error| TelemetryRuntimeError::exporter("http_client", error))?;
        let configured_headers = settings
            .headers
            .iter()
            .map(|(name, value)| {
                let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| TelemetryRuntimeError::Secret("secret header name is invalid"))?;
                let value = reqwest::header::HeaderValue::from_str(value)
                    .map_err(|_| TelemetryRuntimeError::Secret("secret header value is invalid"))?;
                Ok((name, value))
            })
            .collect::<Result<Vec<_>, TelemetryRuntimeError>>()?;
        Ok(Self {
            inner,
            configured_headers,
            gate,
            retry: settings.retry,
            diagnostics,
        })
    }
}

#[async_trait::async_trait]
impl HttpClient for GuardedHttpClient {
    async fn send_bytes(&self, request: Request<Bytes>) -> Result<Response<Bytes>, HttpError> {
        let method = request.method().clone();
        let uri = request.uri().to_string();
        let encoded_body = request.body().clone();
        let decoded_body = if request
            .headers()
            .get("content-encoding")
            .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"gzip"))
        {
            let mut decoder = flate2::read::GzDecoder::new(encoded_body.as_ref());
            let mut decoded = Vec::new();
            decoder.read_to_end(&mut decoded)?;
            decoded
        } else {
            encoded_body.to_vec()
        };
        let signal = SignalRequest::from_uri_and_body(&uri, &decoded_body)?;
        let mut headers = reqwest::header::HeaderMap::new();
        for name in ["content-type", "content-encoding", "user-agent"] {
            if let Some(value) = request.headers().get(name) {
                headers.insert(
                    reqwest::header::HeaderName::from_static(name),
                    reqwest::header::HeaderValue::from_bytes(value.as_bytes())?,
                );
            }
        }
        for (name, value) in &self.configured_headers {
            headers.insert(name.clone(), value.clone());
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| Box::new(error) as HttpError)?;
        runtime.block_on(self.send_with_retry(method, uri, headers, encoded_body, signal))
    }
}

impl GuardedHttpClient {
    async fn send_with_retry(
        &self,
        method: reqwest::Method,
        uri: String,
        headers: reqwest::header::HeaderMap,
        body: Bytes,
        signal: SignalRequest,
    ) -> Result<Response<Bytes>, HttpError> {
        let started = Instant::now();
        let mut retries = 0u8;
        loop {
            if !self.gate.is_active() {
                self.diagnostics.locally_dropped(signal.count());
                return Err(Box::new(TransportError("telemetry generation was revoked")));
            }
            self.diagnostics.request_attempt(retries != 0);
            let request = self
                .inner
                .request(method.clone(), &uri)
                .headers(headers.clone())
                .body(body.clone())
                .send();
            let response = tokio::select! {
                response = request => response,
                () = wait_for_revocation(&self.gate) => {
                    self.diagnostics.locally_dropped(signal.count());
                    return Err(Box::new(TransportError("telemetry generation was revoked")));
                }
            };
            match response {
                Ok(response) => {
                    let status = response.status();
                    let retry_after = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|value| value.to_str().ok())
                        .and_then(parse_retry_after);
                    let response_headers = response.headers().clone();
                    let response_body = tokio::select! {
                        response_body = response.bytes() => response_body.map_err(|error| {
                            self.diagnostics.ambiguous(signal.count());
                            Box::new(error) as HttpError
                        })?,
                        () = wait_for_revocation(&self.gate) => {
                            self.diagnostics.locally_dropped(signal.count());
                            return Err(Box::new(TransportError("telemetry generation was revoked")));
                        }
                    };
                    if status.is_success() {
                        let rejected = signal.rejected_count(&response_body).map_err(|error| {
                            self.diagnostics.ambiguous(signal.count());
                            Box::new(error) as HttpError
                        })?;
                        self.diagnostics.acknowledged(
                            signal.count().saturating_sub(rejected),
                            rejected,
                            unix_ms(),
                        );
                        return build_response(status, response_headers, response_body);
                    }
                    if retryable_status(status.as_u16())
                        && retries < self.retry.max_retries
                        && within_budget(started, self.retry.total_budget_ms)
                    {
                        retries += 1;
                        if !sleep_backoff(retries, retry_after, self.retry, started, &self.gate)
                            .await
                        {
                            self.diagnostics.locally_dropped(signal.count());
                            return Err(Box::new(TransportError(
                                "telemetry generation was revoked",
                            )));
                        }
                        continue;
                    }
                    self.diagnostics.locally_dropped(signal.count());
                    return build_response(status, response_headers, response_body);
                }
                Err(error) => {
                    let retryable =
                        (error.is_connect() || error.is_timeout() || error.is_request())
                            && !likely_tls_failure(&error);
                    if retryable
                        && retries < self.retry.max_retries
                        && within_budget(started, self.retry.total_budget_ms)
                    {
                        retries += 1;
                        if !sleep_backoff(retries, None, self.retry, started, &self.gate).await {
                            self.diagnostics.locally_dropped(signal.count());
                            return Err(Box::new(TransportError(
                                "telemetry generation was revoked",
                            )));
                        }
                        continue;
                    }
                    if error.is_connect() && !error.is_timeout() {
                        self.diagnostics.locally_dropped(signal.count());
                    } else {
                        self.diagnostics.ambiguous(signal.count());
                    }
                    return Err(Box::new(error));
                }
            }
        }
    }
}

fn build_response(
    status: reqwest::StatusCode,
    headers: reqwest::header::HeaderMap,
    body: impl AsRef<[u8]>,
) -> Result<Response<Bytes>, HttpError> {
    let mut builder = Response::builder().status(status.as_u16());
    for (name, value) in &headers {
        builder = builder.header(name.as_str(), value.as_bytes());
    }
    Ok(builder.body(Bytes::copy_from_slice(body.as_ref()))?)
}

#[derive(Debug, Clone, Copy)]
enum SignalRequest {
    Trace { count: u64 },
    Metric { count: u64 },
    Log { count: u64 },
}

impl SignalRequest {
    fn from_uri_and_body(uri: &str, body: &[u8]) -> Result<Self, HttpError> {
        if uri.ends_with("/v1/traces") {
            let request = ExportTraceServiceRequest::decode(body)?;
            let count = request
                .resource_spans
                .iter()
                .flat_map(|resource| &resource.scope_spans)
                .map(|scope| scope.spans.len() as u64)
                .sum();
            return Ok(Self::Trace { count });
        }
        if uri.ends_with("/v1/logs") {
            let request = ExportLogsServiceRequest::decode(body)?;
            let count = request
                .resource_logs
                .iter()
                .flat_map(|resource| &resource.scope_logs)
                .map(|scope| scope.log_records.len() as u64)
                .sum();
            return Ok(Self::Log { count });
        }
        if uri.ends_with("/v1/metrics") {
            let request = ExportMetricsServiceRequest::decode(body)?;
            let count = request
                .resource_metrics
                .iter()
                .flat_map(|resource| &resource.scope_metrics)
                .flat_map(|scope| &scope.metrics)
                .map(metric_data_point_count)
                .sum();
            return Ok(Self::Metric { count });
        }
        Err(Box::new(TransportError("unknown OTLP signal path")))
    }

    fn count(self) -> u64 {
        match self {
            Self::Trace { count } | Self::Metric { count } | Self::Log { count } => count,
        }
    }

    fn rejected_count(self, body: &[u8]) -> Result<u64, prost::DecodeError> {
        match self {
            Self::Trace { .. } => Ok(ExportTraceServiceResponse::decode(body)?
                .partial_success
                .map(|value| value.rejected_spans.max(0) as u64)
                .unwrap_or(0)),
            Self::Metric { .. } => Ok(ExportMetricsServiceResponse::decode(body)?
                .partial_success
                .map(|value| value.rejected_data_points.max(0) as u64)
                .unwrap_or(0)),
            Self::Log { .. } => Ok(ExportLogsServiceResponse::decode(body)?
                .partial_success
                .map(|value| value.rejected_log_records.max(0) as u64)
                .unwrap_or(0)),
        }
    }
}

fn metric_data_point_count(metric: &opentelemetry_proto::tonic::metrics::v1::Metric) -> u64 {
    use opentelemetry_proto::tonic::metrics::v1::metric::Data;
    match metric.data.as_ref() {
        Some(Data::Gauge(value)) => value.data_points.len() as u64,
        Some(Data::Sum(value)) => value.data_points.len() as u64,
        Some(Data::Histogram(value)) => value.data_points.len() as u64,
        Some(Data::ExponentialHistogram(value)) => value.data_points.len() as u64,
        Some(Data::Summary(value)) => value.data_points.len() as u64,
        None => 0,
    }
}

fn retryable_status(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504)
}

fn parse_retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

async fn sleep_backoff(
    retry: u8,
    retry_after: Option<Duration>,
    config: TelemetryRetryConfig,
    started: Instant,
    gate: &GenerationGate,
) -> bool {
    let exponent = u32::from(retry.saturating_sub(1)).min(20);
    let ceiling = config
        .initial_delay_ms
        .saturating_mul(1u64 << exponent)
        .min(config.max_delay_ms);
    let jitter = rand::thread_rng().gen_range(0..=ceiling);
    let delay = retry_after
        .unwrap_or_else(|| Duration::from_millis(jitter))
        .min(Duration::from_millis(config.max_delay_ms));
    let remaining = Duration::from_millis(config.total_budget_ms).saturating_sub(started.elapsed());
    tokio::select! {
        () = tokio::time::sleep(delay.min(remaining)) => gate.is_active(),
        () = wait_for_revocation(gate) => false,
    }
}

async fn wait_for_revocation(gate: &GenerationGate) {
    while gate.is_active() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn within_budget(started: Instant, budget_ms: u64) -> bool {
    started.elapsed() < Duration::from_millis(budget_ms)
}

fn likely_tls_failure(error: &reqwest::Error) -> bool {
    let mut source = error.source();
    while let Some(current) = source {
        let message = current.to_string().to_ascii_lowercase();
        if message.contains("certificate") || message.contains("tls") || message.contains("rustls")
        {
            return true;
        }
        source = current.source();
    }
    false
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[derive(Debug)]
struct TransportError(&'static str);

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for TransportError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{OtlpCompression, TelemetryBatchConfig, TelemetrySamplingConfig};
    use bitfun_observability::{DeploymentEnvironment, ReleaseChannel, SignalPolicy};
    use opentelemetry_proto::tonic::collector::trace::v1::ExportTracePartialSuccess;
    use opentelemetry_proto::tonic::common::v1::InstrumentationScope;
    use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
    use opentelemetry_proto::tonic::metrics::v1::{
        metric, Gauge, Metric, NumberDataPoint, ResourceMetrics, ScopeMetrics,
    };
    use opentelemetry_proto::tonic::resource::v1::Resource;
    use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
    use std::collections::{BTreeMap, HashMap};
    use std::io::Write as _;
    use std::net::TcpListener;
    use std::sync::{Condvar, Mutex};

    #[derive(Clone)]
    struct ResponsePlan {
        status: u16,
        body: Vec<u8>,
        retry_after: Option<&'static str>,
        delay: Duration,
    }

    impl ResponsePlan {
        fn status(status: u16) -> Self {
            Self {
                status,
                body: Vec::new(),
                retry_after: None,
                delay: Duration::ZERO,
            }
        }
    }

    #[derive(Debug, Clone)]
    struct CapturedRequest {
        path: String,
        headers: BTreeMap<String, String>,
        body: Vec<u8>,
    }

    struct TestCollector {
        endpoint: String,
        captured: Arc<(Mutex<Vec<CapturedRequest>>, Condvar)>,
        worker: Option<std::thread::JoinHandle<()>>,
    }

    impl TestCollector {
        fn start(plans: Vec<ResponsePlan>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let captured = Arc::new((Mutex::new(Vec::new()), Condvar::new()));
            let worker_captured = captured.clone();
            let worker = std::thread::spawn(move || {
                for plan in plans {
                    let (mut stream, _) = listener.accept().unwrap();
                    let request = read_request(&mut stream);
                    {
                        let (requests, changed) = &*worker_captured;
                        requests.lock().unwrap().push(request);
                        changed.notify_all();
                    }
                    if !plan.delay.is_zero() {
                        std::thread::sleep(plan.delay);
                    }
                    let reason = match plan.status {
                        200 => "OK",
                        400 => "Bad Request",
                        429 => "Too Many Requests",
                        502 => "Bad Gateway",
                        503 => "Service Unavailable",
                        504 => "Gateway Timeout",
                        _ => "Test Response",
                    };
                    let retry_after = plan
                        .retry_after
                        .map_or_else(String::new, |value| format!("Retry-After: {value}\r\n"));
                    let response = format!(
                        "HTTP/1.1 {} {}\r\nContent-Type: application/x-protobuf\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n",
                        plan.status,
                        reason,
                        plan.body.len(),
                        retry_after,
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.write_all(&plan.body);
                }
            });
            Self {
                endpoint,
                captured,
                worker: Some(worker),
            }
        }

        fn wait_for_requests(&self, count: usize, timeout: Duration) -> Vec<CapturedRequest> {
            let deadline = Instant::now() + timeout;
            let (requests, changed) = &*self.captured;
            let mut requests = requests.lock().unwrap();
            while requests.len() < count {
                let remaining = deadline.saturating_duration_since(Instant::now());
                assert!(
                    !remaining.is_zero(),
                    "collector did not receive {count} requests"
                );
                let (next, wait) = changed.wait_timeout(requests, remaining).unwrap();
                requests = next;
                assert!(!(wait.timed_out() && requests.len() < count));
            }
            requests.clone()
        }
    }

    impl Drop for TestCollector {
        fn drop(&mut self) {
            if let Some(worker) = self.worker.take() {
                worker.join().unwrap();
            }
        }
    }

    fn read_request(stream: &mut std::net::TcpStream) -> CapturedRequest {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 4096];
        let header_end = loop {
            let read = stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0);
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(index) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let header_text = std::str::from_utf8(&bytes[..header_end]).unwrap();
        let mut lines = header_text.split("\r\n");
        let path = lines
            .next()
            .unwrap()
            .split_ascii_whitespace()
            .nth(1)
            .unwrap()
            .to_string();
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
            .collect::<BTreeMap<_, _>>();
        let content_length = headers
            .get("content-length")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        while bytes.len() < header_end + content_length {
            let read = stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0);
            bytes.extend_from_slice(&buffer[..read]);
        }
        CapturedRequest {
            path,
            headers,
            body: bytes[header_end..header_end + content_length].to_vec(),
        }
    }

    fn settings(endpoint: String, max_retries: u8) -> ValidatedTelemetrySettings {
        ValidatedTelemetrySettings {
            endpoint,
            audience: "test|anonymous".to_string(),
            headers: HashMap::from([("x-test-token".to_string(), "redacted".to_string())]),
            compression: OtlpCompression::Gzip,
            signals: SignalPolicy::all(),
            batch: TelemetryBatchConfig {
                request_timeout_ms: 2_000,
                ..TelemetryBatchConfig::default()
            },
            sampling: TelemetrySamplingConfig::default(),
            retry: TelemetryRetryConfig {
                max_retries,
                initial_delay_ms: 1,
                max_delay_ms: 1,
                total_budget_ms: 2_000,
            },
            environment: DeploymentEnvironment::Test,
            release_channel: ReleaseChannel::Development,
        }
    }

    fn make_client(
        settings: &ValidatedTelemetrySettings,
    ) -> (
        GuardedHttpClient,
        Arc<GenerationGate>,
        Arc<TransportDiagnostics>,
    ) {
        let gate = Arc::new(GenerationGate::new());
        let diagnostics = Arc::new(TransportDiagnostics::default());
        (
            GuardedHttpClient::new(settings, gate.clone(), diagnostics.clone()).unwrap(),
            gate,
            diagnostics,
        )
    }

    fn gzip(body: Vec<u8>) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&body).unwrap();
        encoder.finish().unwrap()
    }

    fn request(uri: String, body: Vec<u8>) -> Request<Bytes> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/x-protobuf")
            .header("content-encoding", "gzip")
            .body(Bytes::from(gzip(body)))
            .unwrap()
    }

    fn trace_request(span_count: usize) -> Vec<u8> {
        ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource::default()),
                scope_spans: vec![ScopeSpans {
                    scope: Some(InstrumentationScope::default()),
                    spans: vec![Span::default(); span_count],
                    ..ScopeSpans::default()
                }],
                ..ResourceSpans::default()
            }],
        }
        .encode_to_vec()
    }

    fn log_request() -> Vec<u8> {
        ExportLogsServiceRequest {
            resource_logs: vec![ResourceLogs {
                resource: Some(Resource::default()),
                scope_logs: vec![ScopeLogs {
                    scope: Some(InstrumentationScope::default()),
                    log_records: vec![LogRecord::default()],
                    ..ScopeLogs::default()
                }],
                ..ResourceLogs::default()
            }],
        }
        .encode_to_vec()
    }

    fn metric_request() -> Vec<u8> {
        ExportMetricsServiceRequest {
            resource_metrics: vec![ResourceMetrics {
                resource: Some(Resource::default()),
                scope_metrics: vec![ScopeMetrics {
                    scope: Some(InstrumentationScope::default()),
                    metrics: vec![Metric {
                        data: Some(metric::Data::Gauge(Gauge {
                            data_points: vec![NumberDataPoint::default()],
                        })),
                        ..Metric::default()
                    }],
                    ..ScopeMetrics::default()
                }],
                ..ResourceMetrics::default()
            }],
        }
        .encode_to_vec()
    }

    #[test]
    fn sends_all_standard_http_paths_with_gzip_and_configured_headers() {
        let collector = TestCollector::start(vec![ResponsePlan::status(200); 3]);
        let settings = settings(collector.endpoint.clone(), 0);
        let (client, _, diagnostics) = make_client(&settings);
        for (path, body) in [
            ("/v1/traces", trace_request(1)),
            ("/v1/metrics", metric_request()),
            ("/v1/logs", log_request()),
        ] {
            let response = futures::executor::block_on(
                client.send_bytes(request(format!("{}{path}", collector.endpoint), body)),
            )
            .unwrap();
            assert_eq!(response.status(), 200);
        }

        let captured = collector.wait_for_requests(3, Duration::from_secs(2));
        assert_eq!(
            captured
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            ["/v1/traces", "/v1/metrics", "/v1/logs"]
        );
        for request in &captured {
            assert_eq!(
                request.headers.get("content-encoding").map(String::as_str),
                Some("gzip")
            );
            assert_eq!(
                request.headers.get("x-test-token").map(String::as_str),
                Some("redacted")
            );
            assert_eq!(request.body.get(..2), Some(&[0x1f, 0x8b][..]));
        }
        assert_eq!(diagnostics.snapshot().acknowledged, 3);
    }

    #[test]
    fn records_partial_success_without_replaying_the_batch() {
        let response = ExportTraceServiceResponse {
            partial_success: Some(ExportTracePartialSuccess {
                rejected_spans: 1,
                error_message: "one invalid test span".to_string(),
            }),
        }
        .encode_to_vec();
        let collector = TestCollector::start(vec![ResponsePlan {
            body: response,
            ..ResponsePlan::status(200)
        }]);
        let settings = settings(collector.endpoint.clone(), 8);
        let (client, _, diagnostics) = make_client(&settings);
        futures::executor::block_on(client.send_bytes(request(
            format!("{}/v1/traces", collector.endpoint),
            trace_request(2),
        )))
        .unwrap();

        assert_eq!(
            collector.wait_for_requests(1, Duration::from_secs(2)).len(),
            1
        );
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.acknowledged, 1);
        assert_eq!(snapshot.server_rejected, 1);
        assert_eq!(snapshot.retry_attempts, 0);
    }

    #[test]
    fn retries_only_retryable_statuses_and_honors_attempt_limit() {
        let retrying = TestCollector::start(vec![
            ResponsePlan {
                retry_after: Some("0"),
                ..ResponsePlan::status(503)
            },
            ResponsePlan::status(429),
            ResponsePlan::status(200),
        ]);
        let retry_settings = settings(retrying.endpoint.clone(), 8);
        let (client, _, diagnostics) = make_client(&retry_settings);
        futures::executor::block_on(client.send_bytes(request(
            format!("{}/v1/traces", retrying.endpoint),
            trace_request(1),
        )))
        .unwrap();
        assert_eq!(
            retrying.wait_for_requests(3, Duration::from_secs(2)).len(),
            3
        );
        assert_eq!(diagnostics.snapshot().retry_attempts, 2);

        let rejected = TestCollector::start(vec![ResponsePlan::status(400)]);
        let reject_settings = settings(rejected.endpoint.clone(), 8);
        let (client, _, diagnostics) = make_client(&reject_settings);
        let response = futures::executor::block_on(client.send_bytes(request(
            format!("{}/v1/traces", rejected.endpoint),
            trace_request(1),
        )))
        .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(
            rejected.wait_for_requests(1, Duration::from_secs(2)).len(),
            1
        );
        assert_eq!(diagnostics.snapshot().retry_attempts, 0);

        let limited = TestCollector::start(vec![ResponsePlan::status(503); 9]);
        let limit_settings = settings(limited.endpoint.clone(), 8);
        let (client, _, diagnostics) = make_client(&limit_settings);
        let response = futures::executor::block_on(client.send_bytes(request(
            format!("{}/v1/traces", limited.endpoint),
            trace_request(1),
        )))
        .unwrap();
        assert_eq!(response.status(), 503);
        assert_eq!(
            limited.wait_for_requests(9, Duration::from_secs(2)).len(),
            9
        );
        assert_eq!(diagnostics.snapshot().retry_attempts, 8);
    }

    #[test]
    fn redirects_are_not_followed_or_retried() {
        let collector = TestCollector::start(vec![ResponsePlan {
            status: 302,
            body: Vec::new(),
            retry_after: None,
            delay: Duration::ZERO,
        }]);
        let settings = settings(collector.endpoint.clone(), 8);
        let (client, _gate, diagnostics) = make_client(&settings);

        let result = futures::executor::block_on(client.send_bytes(request(
            format!("{}/v1/traces", collector.endpoint),
            trace_request(1),
        )));

        assert_eq!(result.unwrap().status(), 302);
        assert_eq!(
            collector.wait_for_requests(1, Duration::from_secs(1)).len(),
            1
        );
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.retry_attempts, 0);
        assert_eq!(snapshot.locally_dropped, 1);
    }

    #[test]
    fn invalid_success_response_is_ambiguous_and_not_retried() {
        let collector = TestCollector::start(vec![ResponsePlan {
            status: 200,
            body: vec![0xff],
            retry_after: None,
            delay: Duration::ZERO,
        }]);
        let settings = settings(collector.endpoint.clone(), 8);
        let (client, _gate, diagnostics) = make_client(&settings);

        let result = futures::executor::block_on(client.send_bytes(request(
            format!("{}/v1/traces", collector.endpoint),
            trace_request(1),
        )));

        assert!(result.is_err());
        assert_eq!(
            collector.wait_for_requests(1, Duration::from_secs(1)).len(),
            1
        );
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.retry_attempts, 0);
        assert_eq!(snapshot.ambiguous, 1);
        assert_eq!(snapshot.locally_dropped, 0);
    }

    #[test]
    fn terminal_timeout_is_ambiguous_and_not_double_counted() {
        let collector = TestCollector::start(vec![ResponsePlan {
            status: 200,
            body: ExportTraceServiceResponse::default().encode_to_vec(),
            retry_after: None,
            delay: Duration::from_millis(100),
        }]);
        let mut settings = settings(collector.endpoint.clone(), 0);
        settings.batch.request_timeout_ms = 10;
        let (client, _gate, diagnostics) = make_client(&settings);

        let result = futures::executor::block_on(client.send_bytes(request(
            format!("{}/v1/traces", collector.endpoint),
            trace_request(1),
        )));

        assert!(result.is_err());
        assert_eq!(
            collector.wait_for_requests(1, Duration::from_secs(1)).len(),
            1
        );
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.retry_attempts, 0);
        assert_eq!(snapshot.ambiguous, 1);
        assert_eq!(snapshot.locally_dropped, 0);
    }

    #[test]
    fn generation_revocation_cancels_an_in_flight_request() {
        let collector = TestCollector::start(vec![ResponsePlan {
            delay: Duration::from_secs(1),
            ..ResponsePlan::status(200)
        }]);
        let settings = settings(collector.endpoint.clone(), 0);
        let (client, gate, diagnostics) = make_client(&settings);
        let endpoint = collector.endpoint.clone();
        let started = Instant::now();
        let sender = std::thread::spawn(move || {
            futures::executor::block_on(
                client.send_bytes(request(format!("{endpoint}/v1/traces"), trace_request(1))),
            )
        });
        collector.wait_for_requests(1, Duration::from_secs(2));
        gate.deactivate();
        assert!(sender.join().unwrap().is_err());
        assert!(started.elapsed() < Duration::from_millis(500));
        assert_eq!(diagnostics.snapshot().locally_dropped, 1);
    }
}
