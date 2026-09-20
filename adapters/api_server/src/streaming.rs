use std::net::SocketAddr;

use axum::{Router, http::StatusCode, routing::get};

use crate::state::ServerState;

pub fn build_streaming_router<H, T>(handler: H) -> Router<ServerState>
where
    H: axum::handler::Handler<T, ServerState>,
    T: 'static,
{
    Router::new().route("/v1/streaming", get(handler))
}

pub fn authorize_streaming_request(
    state: &ServerState,
    addr: SocketAddr,
    token: Option<&str>,
) -> Result<tokio::sync::OwnedSemaphorePermit, StatusCode> {
    let ip = addr.ip().to_canonical();
    let has_valid_api_key = !state.api_key.is_empty() && token == Some(state.api_key.as_str());

    if !has_valid_api_key {
        if !state.ip_whitelist.iter().any(|net| net.contains(&ip)) {
            log::warn!(
                "[ApiServer] Streaming request from {} rejected: IP not in whitelist ({:?})",
                ip,
                state.ip_whitelist
            );
            return Err(StatusCode::FORBIDDEN);
        }

        if !state.api_key.is_empty() {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    state
        .streaming_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}
