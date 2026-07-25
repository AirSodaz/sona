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
    if !state
        .ip_whitelist
        .iter()
        .any(|net| net.contains(&addr.ip()))
    {
        return Err(StatusCode::FORBIDDEN);
    }

    if !state.api_key.is_empty() && token.unwrap_or_default() != state.api_key {
        return Err(StatusCode::UNAUTHORIZED);
    }

    state
        .streaming_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}
