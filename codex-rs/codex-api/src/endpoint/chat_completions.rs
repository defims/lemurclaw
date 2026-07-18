//! Chat Completions endpoint client.
//!
//! Mirrors [`crate::endpoint::ResponsesClient`] but targets the OpenAI
//! Chat Completions API (`POST /chat/completions`) and uses the Chat-
//! Completions-specific SSE processor in [`crate::sse::chat_completions`].
//!
//! The request body is a plain JSON value built by the caller (see
//! `codex_core::chat_completions::build_chat_completions_payload`), which
//! translates codex's [`codex_protocol::models::ResponseItem`] history into
//! the `messages` array shape the Chat Completions API expects.

use crate::auth::SharedAuthProvider;
use crate::common::ResponseStream;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::sse::spawn_chat_completions_stream;
use crate::telemetry::SseTelemetry;
use codex_client::EncodedJsonBody;
use codex_client::HttpTransport;
use codex_client::RequestCompression;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use serde_json::Value;
use std::sync::Arc;
use tracing::instrument;

/// Options for a Chat Completions streaming request.
#[derive(Default)]
pub struct ChatCompletionsOptions {
    pub extra_headers: HeaderMap,
}

/// Client for the Chat Completions streaming endpoint.
///
/// This is the third-party-provider counterpart to [`ResponsesClient`]: it
/// re-uses the same transport + auth wiring (so API keys, ChatGPT bearer
/// tokens, custom headers, query params, and retry policy all keep working),
/// but emits a Chat Completions request body and parses the Chat Completions
/// streaming wire format.
pub struct ChatCompletionsClient<T: HttpTransport> {
    session: EndpointSession<T>,
    sse_telemetry: Option<Arc<dyn SseTelemetry>>,
}

impl<T: HttpTransport> ChatCompletionsClient<T> {
    pub fn new(transport: T, provider: Provider, auth: SharedAuthProvider) -> Self {
        Self {
            session: EndpointSession::new(transport, provider, auth),
            sse_telemetry: None,
        }
    }

    pub fn with_telemetry(
        self,
        request: Option<Arc<dyn codex_client::RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        Self {
            session: self.session.with_request_telemetry(request),
            sse_telemetry: sse,
        }
    }

    /// Path appended to the provider base URL for Chat Completions requests.
    /// Overrideable via `ChatCompletionsOptions` is intentionally not provided
    /// — the path is part of the OpenAI Chat Completions contract.
    fn path() -> &'static str {
        "chat/completions"
    }

    /// Stream a Chat Completions request. `body` must be a fully-formed Chat
    /// Completions payload (i.e. already contain `model`, `messages`,
    /// `tools`, `stream: true`, ...).
    #[instrument(
        name = "chat_completions.stream",
        level = "info",
        skip_all,
        fields(
            transport = "chat_completions_http",
            http.method = "POST",
            api.path = "chat/completions"
        )
    )]
    pub async fn stream(
        &self,
        body: Value,
        options: ChatCompletionsOptions,
    ) -> Result<ResponseStream, ApiError> {
        let encoded = EncodedJsonBody::encode(&body)
            .map_err(|e| ApiError::Stream(format!("failed to encode chat completions request: {e}")))?;

        let stream_response = self
            .session
            .stream_encoded_json_with(
                Method::POST,
                Self::path(),
                options.extra_headers,
                Some(encoded),
                |req| {
                    req.headers.insert(
                        http::header::ACCEPT,
                        HeaderValue::from_static("text/event-stream"),
                    );
                    req.compression = RequestCompression::None;
                },
            )
            .await?;

        Ok(spawn_chat_completions_stream(
            stream_response,
            self.session.provider().stream_idle_timeout,
            self.sse_telemetry.clone(),
        ))
    }
}
