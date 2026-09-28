//! Shared WebSocket streaming transcription utilities.
//!
//! Canonical implementation lives in `sona-api-server` as part of the shared
//! inbound HTTP/WebSocket adapter layer.

pub use sona_api_server::{
    ClientMessage, ServerMessage, authorize_streaming_request, build_streaming_router,
    handle_streaming_websocket, resolve_punctuation_model_path, resolve_vad_model_path,
    serialize_server_message,
};
