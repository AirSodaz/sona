use std::time::Duration;

use reqwest::{
    Client, StatusCode, Url,
    header::{HeaderMap, RETRY_AFTER},
};
use serde_json::Value;
use sona_core::llm::provider_protocol::join_url;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

pub(crate) fn classify_llm_port_error(message: impl Into<String>) -> LlmPortError {
    let message = message.into();
    let normalized = message.to_ascii_lowercase();
    let kind = if normalized.contains("401") || normalized.contains("authentication") {
        LlmPortErrorKind::Authentication
    } else if normalized.contains("403") || normalized.contains("permission") {
        LlmPortErrorKind::Permission
    } else if normalized.contains("429") || normalized.contains("rate limit") {
        LlmPortErrorKind::RateLimited
    } else if normalized.contains("timed out") || normalized.contains("timeout") {
        LlmPortErrorKind::Timeout
    } else if normalized.contains("500")
        || normalized.contains("502")
        || normalized.contains("503")
        || normalized.contains("504")
    {
        LlmPortErrorKind::Unavailable
    } else if normalized.contains("connection")
        || normalized.contains("connect")
        || normalized.contains("dns")
        || normalized.contains("error sending request")
        || normalized.contains("failed to lookup address")
    {
        LlmPortErrorKind::Network
    } else if normalized.contains("unsupported") || normalized.contains("does not support") {
        LlmPortErrorKind::Unsupported
    } else if normalized.contains("api host") || normalized.contains("cannot be empty") {
        LlmPortErrorKind::InvalidRequest
    } else {
        LlmPortErrorKind::Protocol
    };
    LlmPortError::new(kind, message)
}

fn reqwest_port_error(error: reqwest::Error) -> LlmPortError {
    let kind = if error.is_timeout() {
        LlmPortErrorKind::Timeout
    } else if error.is_connect() || error.is_request() {
        LlmPortErrorKind::Network
    } else {
        LlmPortErrorKind::Protocol
    };
    LlmPortError::new(kind, error.to_string())
}

fn http_status_port_error(status: StatusCode, headers: &HeaderMap, body: String) -> LlmPortError {
    let kind = match status {
        StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND | StatusCode::UNPROCESSABLE_ENTITY => {
            LlmPortErrorKind::InvalidRequest
        }
        StatusCode::UNAUTHORIZED => LlmPortErrorKind::Authentication,
        StatusCode::FORBIDDEN => LlmPortErrorKind::Permission,
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => LlmPortErrorKind::Timeout,
        StatusCode::TOO_MANY_REQUESTS => LlmPortErrorKind::RateLimited,
        status if status.is_server_error() => LlmPortErrorKind::Unavailable,
        _ => LlmPortErrorKind::Protocol,
    };
    let retry_after_ms = headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1000));
    LlmPortError {
        kind,
        message: format!("LLM API Error: {status} {body}"),
        retry_after_ms,
    }
}

pub fn parse_llm_api_host(base_url: &str) -> Result<Url, String> {
    let trimmed = base_url.trim();
    validate_llm_api_host(trimmed)?;
    Url::parse(trimmed).map_err(|error| format!("LLM API host is invalid: {error}"))
}

pub fn validate_llm_api_host(base_url: &str) -> Result<(), String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err("LLM API host cannot be empty".to_string());
    }

    let url = Url::parse(trimmed).map_err(|error| format!("LLM API host is invalid: {error}"))?;
    match url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback_host(&url) => Ok(()),
        "http" => Err("LLM API host must use https:// unless it points to localhost.".to_string()),
        _ => Err("LLM API host must start with https:// or localhost http://.".to_string()),
    }
}

fn is_loopback_host(url: &Url) -> bool {
    url.host_str()
        .map(|host| {
            let normalized = host.trim_matches(['[', ']']).to_ascii_lowercase();
            normalized == "localhost" || normalized == "127.0.0.1" || normalized == "::1"
        })
        .unwrap_or(false)
}

#[derive(Clone, Debug)]
pub struct LlmApiUrl {
    value: Url,
    https_only: bool,
}

impl LlmApiUrl {
    pub fn parse(value: &str) -> Result<Self, String> {
        let url = parse_llm_api_host(value)?;
        let https_only = url.scheme() == "https";
        Ok(Self {
            value: url,
            https_only,
        })
    }

    pub fn as_str(&self) -> &str {
        self.value.as_str()
    }

    pub fn reqwest_url(&self) -> Url {
        self.value.clone()
    }

    pub fn join(&self, path: &str) -> Result<Self, String> {
        let joined = join_url(self.value.as_str(), path);
        Self::parse(&joined)
    }

    pub fn with_query(&self, query: &str) -> Result<Self, String> {
        let mut url = self.value.clone();
        url.set_query(Some(query));
        Self::parse(url.as_str())
    }

    pub fn client(&self, timeout_seconds: Option<u64>) -> Result<Client, String> {
        use std::collections::HashMap;
        use std::sync::{Mutex, OnceLock};

        type ClientKey = (bool, Option<u64>);
        static CLIENTS: OnceLock<Mutex<HashMap<ClientKey, Client>>> = OnceLock::new();
        let map = CLIENTS.get_or_init(|| Mutex::new(HashMap::new()));

        let key = (self.https_only, timeout_seconds);

        {
            let lock = map.lock().unwrap();
            if let Some(client) = lock.get(&key) {
                return Ok(client.clone());
            }
        }

        let mut builder = Client::builder();
        if self.https_only {
            builder = builder.https_only(true);
        }
        if let Some(secs) = timeout_seconds {
            builder = builder.timeout(Duration::from_secs(secs));
        }
        let client = builder.build().map_err(|error| error.to_string())?;

        let mut lock = map.lock().unwrap();
        lock.insert(key, client.clone());
        Ok(client)
    }
}

pub async fn post_json_request(
    url: &LlmApiUrl,
    headers: Vec<(&str, String)>,
    body: Value,
    timeout_seconds: Option<u64>,
) -> Result<Value, LlmPortError> {
    let client = url
        .client(timeout_seconds)
        .map_err(classify_llm_port_error)?;
    let mut request = client
        .post(url.reqwest_url())
        .header("Content-Type", "application/json");

    for (key, value) in headers {
        if !value.is_empty() {
            request = request.header(key, value);
        }
    }

    let response = request
        .json(&body)
        .send()
        .await
        .map_err(reqwest_port_error)?;

    let status = response.status();
    let headers = response.headers().clone();
    let text = response.text().await.map_err(reqwest_port_error)?;

    if !status.is_success() {
        return Err(http_status_port_error(status, &headers, text));
    }

    serde_json::from_str(&text)
        .map_err(|error| LlmPortError::new(LlmPortErrorKind::Protocol, error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_network_failures() {
        assert_eq!(
            classify_llm_port_error("error sending request: connection refused").kind,
            LlmPortErrorKind::Network
        );
    }
}
