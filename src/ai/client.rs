//! HTTP 客户端 — ureq + 重试 + 超时。
//!
//! 超时 1800 秒（适配长推理模型）。重试 3 次，指数退避 1s/2s/4s。
//! 仅重试网络错误、5xx、429。4xx（除 429）不重试。

use std::io::Read as _;
use std::time::Duration;

use super::{AiError, AiErrorKind};

/// HTTP 请求描述（provider 构造，client 执行）。
#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// HTTP 响应。
#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// 可注入的请求执行器（测试用 mock）。
pub type RequestExecutor = Box<dyn Fn(&HttpRequest) -> Result<HttpResponse, AiError> + Send + Sync>;

/// 默认执行器：用 ureq 发真实 HTTP 请求。
fn default_executor(timeout: Duration) -> RequestExecutor {
    Box::new(move |req: &HttpRequest| -> Result<HttpResponse, AiError> {
        let agent = ureq::AgentBuilder::new().timeout(timeout).build();
        let mut request = match req.method.as_str() {
            "GET" => agent.get(&req.url),
            _ => agent.post(&req.url),
        };
        for (k, v) in &req.headers {
            request = request.set(k, v);
        }
        match if req.body.is_empty() {
            request.call()
        } else {
            request.send_string(&req.body)
        } {
            Ok(resp) => {
                let status = resp.status();
                let mut body = String::new();
                resp.into_reader().read_to_string(&mut body).map_err(|e| {
                    AiError::new(AiErrorKind::Network, format!("read body: {e}"), None)
                })?;
                Ok(HttpResponse { status, body })
            }
            Err(ureq::Error::Status(code, resp)) => {
                let mut body = String::new();
                let _ = resp.into_reader().read_to_string(&mut body);
                let kind = if code == 401 || code == 403 {
                    AiErrorKind::Auth
                } else if code == 429 {
                    AiErrorKind::RateLimit
                } else if code >= 500 {
                    AiErrorKind::ServerError
                } else {
                    AiErrorKind::Other
                };
                Err(AiError::new(
                    kind,
                    format!("HTTP {code}: {body}"),
                    Some(code),
                ))
            }
            Err(ureq::Error::Transport(e)) => Err(AiError::new(
                AiErrorKind::Network,
                format!("transport: {e}"),
                None,
            )),
        }
    })
}

/// 带重试的 HTTP 请求。
///
/// - `timeout`: 单次请求超时（仅用于默认执行器，mock 执行器忽略）。
/// - `max_retries`: 最大重试次数（总请求数 = 1 + max_retries）。
/// - `backoff`: 首次重试前等待时长（后续指数翻倍）。
/// - `executor`: 请求执行器。
pub fn http_request_with_retry(
    request: HttpRequest,
    timeout: Duration,
    max_retries: u32,
    backoff: Duration,
    executor: RequestExecutor,
) -> Result<HttpResponse, AiError> {
    let _ = timeout; // 已封装在 default_executor 内
    let mut last_err: Option<AiError> = None;
    let mut current_backoff = backoff;
    for attempt in 0..=max_retries {
        if attempt > 0 {
            std::thread::sleep(current_backoff);
            current_backoff = current_backoff.saturating_mul(2);
        }
        match executor(&request) {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                let retryable = matches!(
                    e.kind,
                    AiErrorKind::Network | AiErrorKind::ServerError | AiErrorKind::RateLimit
                );
                if !retryable {
                    return Err(e);
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err
        .unwrap_or_else(|| AiError::new(AiErrorKind::Other, "no attempts made".to_string(), None)))
}

/// 公开入口：用默认 ureq 执行器发带重试的请求。
pub fn request_with_retry(request: HttpRequest) -> Result<HttpResponse, AiError> {
    // 超时 1800 秒，重试 3 次，退避 1 秒起。
    http_request_with_retry(
        request,
        Duration::from_secs(1800),
        3,
        Duration::from_secs(1),
        default_executor(Duration::from_secs(1800)),
    )
}
