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
    let has_status_context = ["http", "status", "response", "api error", "server"]
        .iter()
        .any(|marker| normalized.contains(marker));
    let has_server_status = has_status_context
        && normalized
            .split(|character: char| !character.is_ascii_digit())
            .filter_map(|token| {
                (token.len() == 3)
                    .then(|| token.parse::<u16>().ok())
                    .flatten()
            })
            .any(|status| (500..=599).contains(&status));
    let kind = if normalized.contains("401") || normalized.contains("authentication") {
        LlmPortErrorKind::Authentication
    } else if normalized.contains("403") || normalized.contains("permission") {
        LlmPortErrorKind::Permission
    } else if normalized.contains("429") || normalized.contains("rate limit") {
        LlmPortErrorKind::RateLimited
    } else if normalized.contains("timed out") || normalized.contains("timeout") {
        LlmPortErrorKind::Timeout
    } else if has_server_status {
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

pub(crate) fn reqwest_port_error(error: reqwest::Error) -> LlmPortError {
    let kind = if error.is_timeout() {
        LlmPortErrorKind::Timeout
    } else if error.is_connect() || error.is_request() {
        LlmPortErrorKind::Network
    } else {
        LlmPortErrorKind::Protocol
    };
    LlmPortError::new(kind, error.to_string())
}

pub(crate) fn http_status_port_error(
    status: StatusCode,
    headers: &HeaderMap,
    body: String,
) -> LlmPortError {
    let normalized_body = body.to_ascii_lowercase();
    let streaming_unsupported = normalized_body.contains("stream")
        && (normalized_body.contains("unsupported")
            || normalized_body.contains("not support")
            || normalized_body.contains("does not support"));
    let kind = match status {
        _ if status.is_client_error()
            && status != StatusCode::TOO_MANY_REQUESTS
            && streaming_unsupported =>
        {
            LlmPortErrorKind::Unsupported
        }
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

pub fn parse_llm_api_host(base_url: &str) -> Result<Url, LlmPortError> {
    let trimmed = base_url.trim();
    validate_llm_api_host(trimmed)?;
    Url::parse(trimmed).map_err(|error| {
        LlmPortError::new(
            LlmPortErrorKind::InvalidRequest,
            format!("LLM API host is invalid: {error}"),
        )
    })
}

pub fn validate_llm_api_host(base_url: &str) -> Result<(), LlmPortError> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(LlmPortError::new(
            LlmPortErrorKind::InvalidRequest,
            "LLM API host cannot be empty",
        ));
    }

    let url = Url::parse(trimmed).map_err(|error| {
        LlmPortError::new(
            LlmPortErrorKind::InvalidRequest,
            format!("LLM API host is invalid: {error}"),
        )
    })?;
    match url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback_host(&url) => Ok(()),
        "http" => Err(LlmPortError::new(
            LlmPortErrorKind::InvalidRequest,
            "LLM API host must use https:// unless it points to localhost.",
        )),
        _ => Err(LlmPortError::new(
            LlmPortErrorKind::InvalidRequest,
            "LLM API host must start with https:// or localhost http://.",
        )),
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
    pub fn parse(value: &str) -> Result<Self, LlmPortError> {
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

    pub fn join(&self, path: &str) -> Result<Self, LlmPortError> {
        let joined = join_url(self.value.as_str(), path);
        Self::parse(&joined)
    }

    pub fn with_query(&self, query: &str) -> Result<Self, LlmPortError> {
        let mut url = self.value.clone();
        url.set_query(Some(query));
        Self::parse(url.as_str())
    }

    pub fn client(&self, timeout_seconds: Option<u64>) -> Result<Client, LlmPortError> {
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
        let client = builder.build().map_err(reqwest_port_error)?;

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
    let client = url.client(timeout_seconds)?;
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
        assert_eq!(
            classify_llm_port_error("provider response status: 529").kind,
            LlmPortErrorKind::Unavailable
        );
        assert_eq!(
            classify_llm_port_error("invalid JSON at column 529").kind,
            LlmPortErrorKind::Protocol
        );
    }
}
