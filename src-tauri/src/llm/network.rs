use super::*;
use reqwest::{Client, Url};
use serde_json::Value;

pub(crate) fn parse_llm_api_host(base_url: &str) -> Result<Url, String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err("LLM API host cannot be empty".to_string());
    }

    let url = Url::parse(trimmed).map_err(|error| format!("LLM API host is invalid: {error}"))?;
    match url.scheme() {
        "https" => Ok(url),
        "http" if is_loopback_host(&url) => Ok(url),
        "http" => Err("LLM API host must use https:// unless it points to localhost.".to_string()),
        _ => Err("LLM API host must start with https:// or localhost http://.".to_string()),
    }
}

pub(crate) fn validate_llm_api_host(base_url: &str) -> Result<(), String> {
    parse_llm_api_host(base_url).map(|_| ())
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
pub(crate) struct LlmApiUrl {
    value: Url,
    https_only: bool,
}

impl LlmApiUrl {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let url = parse_llm_api_host(value)?;
        let https_only = url.scheme() == "https";
        Ok(Self {
            value: url,
            https_only,
        })
    }

    pub(crate) fn as_str(&self) -> &str {
        self.value.as_str()
    }

    pub(crate) fn reqwest_url(&self) -> Url {
        self.value.clone()
    }

    pub(crate) fn join(&self, path: &str) -> Result<Self, String> {
        let joined = join_url(self.value.as_str(), path);
        Self::parse(&joined)
    }

    pub(crate) fn with_query(&self, query: &str) -> Result<Self, String> {
        let mut url = self.value.clone();
        url.set_query(Some(query));
        Self::parse(url.as_str())
    }

    pub(crate) fn client(&self) -> Result<Client, String> {
        if self.https_only {
            return Client::builder()
                .https_only(true)
                .build()
                .map_err(|error| error.to_string());
        }

        Ok(Client::new())
    }
}

pub(crate) async fn post_json_request(
    url: &LlmApiUrl,
    headers: Vec<(&str, String)>,
    body: Value,
) -> Result<Value, String> {
    let client = url.client()?;
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
        .map_err(|error| error.to_string())?;

    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;

    if !status.is_success() {
        return Err(format!("LLM API Error: {} {}", status, text));
    }

    serde_json::from_str(&text).map_err(|error| error.to_string())
}
