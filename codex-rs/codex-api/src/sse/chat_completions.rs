//! Chat Completions streaming SSE processor.
//!
//! The Chat Completions API streams a different event shape than the Responses
//! API: each SSE `data:` line is a JSON object of the form
//! `{"id","object":"chat.completion.chunk","model","choices":[{"delta":{...},"finish_reason":...}]}`
//! and the stream terminates with the literal `data: [DONE]`.
//!
//! This module adapts that wire format onto codex's internal [`ResponseEvent`]
//! so the rest of the pipeline can stay agnostic of the underlying wire format.
//! It mirrors `sse::responses` in structure but implements Chat-Completions-
//! specific parsing. The translation logic is adapted from just-every/code's
//! `code-rs/core/src/chat_completions.rs` (`process_chat_sse`).

use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::telemetry::SseTelemetry;
use codex_client::ByteStream;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use codex_protocol::ResponseItemId;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;
use tracing::debug;
use tracing::trace;

/// Convert the optional server-provided id string into a `ResponseItemId`,
/// preserving `None`.
fn item_id_from(id: &Option<String>) -> Option<ResponseItemId> {
    id.as_ref()
        .filter(|id| !id.is_empty())
        .map(|id| ResponseItemId::from_server(id.clone()))
}

/// Spawns a background task that reads Chat Completions SSE chunks from
/// `stream_response.bytes` and forwards codex-native [`ResponseEvent`]s onto a
/// channel, returning a [`ResponseStream`] the caller can poll.
///
/// This is the Chat Completions counterpart of
/// [`crate::spawn_response_stream`].
pub fn spawn_chat_completions_stream(
    stream_response: codex_client::StreamResponse,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) -> ResponseStream {
    let upstream_request_id = stream_response
        .headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);
    tokio::spawn(async move {
        process_chat_sse(
            stream_response.bytes,
            tx_event,
            idle_timeout,
            telemetry,
        )
        .await;
    });

    ResponseStream {
        rx_event,
        upstream_request_id,
    }
}

async fn process_chat_sse(
    stream: ByteStream,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) {
    let mut stream = stream.eventsource();

    // State to accumulate a function call across streaming chunks.
    // OpenAI may split the `arguments` string over multiple `delta` events
    // until the chunk whose `finish_reason` is `tool_calls` is emitted. We
    // keep collecting the pieces here and forward a single
    // `ResponseItem::FunctionCall` once the call is complete.
    #[derive(Default)]
    struct FunctionCallState {
        name: Option<String>,
        arguments: String,
        call_id: Option<String>,
        active: bool,
    }

    let mut fn_call_state = FunctionCallState::default();
    let mut assistant_text = String::new();
    let mut reasoning_text = String::new();
    let mut current_item_id: Option<String> = None;
    let mut current_response_id: Option<String> = None;
    let mut current_response_model: Option<String> = None;
    let mut created_emitted = false;

    loop {
        let start = Instant::now();
        let response = timeout(idle_timeout, stream.next()).await;
        if let Some(t) = telemetry.as_ref() {
            t.on_sse_poll(&response, start.elapsed());
        }
        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("Chat SSE error: {e:#}");
                let _ = tx_event
                    .send(Err(ApiError::Stream(format!("[transport] {e}"))))
                    .await;
                return;
            }
            Ok(None) => {
                // Stream closed by server without an explicit end marker.
                debug!("chat SSE stream closed without [DONE] marker");
                flush_and_complete(
                    &tx_event,
                    &mut assistant_text,
                    &mut reasoning_text,
                    &current_item_id,
                    current_response_id.as_deref(),
                )
                .await;
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream(
                        "[idle] timeout waiting for SSE".into(),
                    )))
                    .await;
                return;
            }
        };

        let data = sse.data.trim();

        if data.is_empty() {
            continue;
        }

        // OpenAI Chat streaming sends a literal string "[DONE]" when finished.
        if data == "[DONE]" || data == "DONE" {
            flush_and_complete(
                &tx_event,
                &mut assistant_text,
                &mut reasoning_text,
                &current_item_id,
                current_response_id.as_deref(),
            )
            .await;
            return;
        }

        let chunk: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(e) => {
                let mut excerpt = sse.data.clone();
                const MAX: usize = 600;
                if excerpt.len() > MAX {
                    excerpt.truncate(MAX);
                }
                debug!("chat SSE parse error: {e} | data: {excerpt}");
                continue;
            }
        };
        trace!("chat_completions received SSE chunk: {chunk:?}");

        if current_response_id.is_none() {
            current_response_id = chunk
                .get("id")
                .and_then(|id| id.as_str())
                .map(ToString::to_string);
        }
        if current_response_model.is_none() {
            current_response_model = chunk
                .get("model")
                .and_then(|model| model.as_str())
                .map(ToString::to_string);
        }
        if !created_emitted
            && (current_response_id.is_some() || current_response_model.is_some())
        {
            let _ = tx_event
                .send(Ok(ResponseEvent::Created))
                .await;
            created_emitted = true;
        }

        // Extract item_id if present at the top level or in choice.
        if let Some(item_id) = chunk.get("item_id").and_then(|id| id.as_str()) {
            current_item_id = Some(item_id.to_string());
        }

        let choice_opt = chunk.get("choices").and_then(|c| c.get(0));

        let Some(choice) = choice_opt else {
            continue;
        };

        // Check for item_id in the choice as well.
        if let Some(item_id) = choice.get("item_id").and_then(|id| id.as_str()) {
            current_item_id = Some(item_id.to_string());
        }

        // Handle assistant content tokens as streaming deltas.
        if let Some(content) = choice
            .get("delta")
            .and_then(|d| d.get("content"))
            .and_then(|c| c.as_str())
        {
            if !content.is_empty() {
                assistant_text.push_str(content);
                let _ = tx_event
                    .send(Ok(ResponseEvent::OutputTextDelta(content.to_string())))
                    .await;
            }
        }

        // Forward any reasoning/thinking deltas if present.
        // Some providers stream `reasoning` as a plain string while others
        // nest the text under an object (e.g. `{ "reasoning": { "text": "…" } }`).
        if let Some(reasoning_val) = choice.get("delta").and_then(|d| d.get("reasoning")) {
            let maybe_text = reasoning_text_delta(reasoning_val);
            if let Some(reasoning) = maybe_text {
                reasoning_text.push_str(&reasoning);
                let _ = tx_event
                    .send(Ok(ResponseEvent::ReasoningContentDelta {
                        delta: reasoning,
                        content_index: 0,
                    }))
                    .await;
            }
        }

        // Some providers only include reasoning on the final message object.
        if let Some(message_reasoning) = choice.get("message").and_then(|m| m.get("reasoning")) {
            if let Some(s) = reasoning_text_from_value(message_reasoning) {
                if !s.is_empty() {
                    reasoning_text.push_str(&s);
                    let _ = tx_event
                        .send(Ok(ResponseEvent::ReasoningContentDelta {
                            delta: s,
                            content_index: 0,
                        }))
                        .await;
                }
            }
        }

        // Handle streaming function / tool calls.
        if let Some(tool_calls) = choice
            .get("delta")
            .and_then(|d| d.get("tool_calls"))
            .and_then(|tc| tc.as_array())
        {
            if let Some(tool_call) = tool_calls.first() {
                // Mark that we have an active function call in progress.
                fn_call_state.active = true;

                // Extract call_id if present.
                if let Some(id) = tool_call.get("id").and_then(|v| v.as_str()) {
                    fn_call_state.call_id.get_or_insert_with(|| id.to_string());
                }

                // Extract function details if present.
                if let Some(function) = tool_call.get("function") {
                    if let Some(name) = function.get("name").and_then(|n| n.as_str()) {
                        fn_call_state.name.get_or_insert_with(|| name.to_string());
                    }

                    if let Some(args_fragment) =
                        function.get("arguments").and_then(|a| a.as_str())
                    {
                        fn_call_state.arguments.push_str(args_fragment);
                    }
                }
            }
        }

        // Emit end-of-turn when finish_reason signals completion.
        let Some(finish_reason) = choice.get("finish_reason").and_then(|v| v.as_str()) else {
            continue;
        };

        match finish_reason {
            "tool_calls" if fn_call_state.active => {
                // First, flush the terminal raw reasoning so UIs can finalize
                // the reasoning stream before any exec/tool events begin.
                if !reasoning_text.is_empty() {
                    let item = ResponseItem::Reasoning {
                        id: item_id_from(&current_item_id),
                        summary: Vec::new(),
                        content: Some(vec![ReasoningItemContent::ReasoningText {
                            text: std::mem::take(&mut reasoning_text),
                        }]),
                        encrypted_content: None,
                        internal_chat_message_metadata_passthrough: None,
                    };
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(item)))
                        .await;
                }

                // Then emit the FunctionCall response item.
                let item = ResponseItem::FunctionCall {
                    id: item_id_from(&current_item_id),
                    name: fn_call_state.name.take().unwrap_or_default(),
                    namespace: None,
                    arguments: std::mem::take(&mut fn_call_state.arguments),
                    call_id: fn_call_state.call_id.take().unwrap_or_default(),
                    internal_chat_message_metadata_passthrough: None,
                };
                let _ = tx_event
                    .send(Ok(ResponseEvent::OutputItemDone(item)))
                    .await;
            }
            "stop" => {
                // Regular turn without tool-call. Emit the final assistant
                // message as a single OutputItemDone.
                if !assistant_text.is_empty() {
                    let item = ResponseItem::Message {
                        role: "assistant".to_string(),
                        content: vec![ContentItem::OutputText {
                            text: std::mem::take(&mut assistant_text),
                        }],
                        id: item_id_from(&current_item_id),
                        phase: None,
                        internal_chat_message_metadata_passthrough: None,
                    };
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(item)))
                        .await;
                }
                if !reasoning_text.is_empty() {
                    let item = ResponseItem::Reasoning {
                        id: item_id_from(&current_item_id),
                        summary: Vec::new(),
                        content: Some(vec![ReasoningItemContent::ReasoningText {
                            text: std::mem::take(&mut reasoning_text),
                        }]),
                        encrypted_content: None,
                        internal_chat_message_metadata_passthrough: None,
                    };
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(item)))
                        .await;
                }
            }
            _ => {}
        }

        // Emit Completed regardless of reason so the agent can advance.
        let _ = tx_event
            .send(Ok(ResponseEvent::Completed {
                response_id: current_response_id.clone().unwrap_or_default(),
                token_usage: None,
                end_turn: Some(finish_reason == "stop"),
            }))
            .await;
        return;
    }
}

/// Emit any finalized items before closing so downstream consumers receive
/// terminal events for both assistant content and raw reasoning, followed by
/// `Completed`. This handles the case where the stream ends (either via
/// `[DONE]` or an unannounced close) without a `finish_reason`.
async fn flush_and_complete(
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
    assistant_text: &mut String,
    reasoning_text: &mut String,
    current_item_id: &Option<String>,
    response_id: Option<&str>,
) {
    if !assistant_text.is_empty() {
        let item = ResponseItem::Message {
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: std::mem::take(assistant_text),
            }],
            id: item_id_from(current_item_id),
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        };
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputItemDone(item)))
            .await;
    }

    if !reasoning_text.is_empty() {
        let item = ResponseItem::Reasoning {
            id: item_id_from(current_item_id),
            summary: Vec::new(),
            content: Some(vec![ReasoningItemContent::ReasoningText {
                text: std::mem::take(reasoning_text),
            }]),
            encrypted_content: None,
            internal_chat_message_metadata_passthrough: None,
        };
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputItemDone(item)))
            .await;
    }

    let _ = tx_event
        .send(Ok(ResponseEvent::Completed {
            response_id: response_id.unwrap_or_default().to_string(),
            token_usage: None,
            end_turn: None,
        }))
        .await;
}

/// Extract a non-empty reasoning delta from a `delta.reasoning` field, which
/// may be either a plain string or an object like `{ "text": "..." }` or
/// `{ "content": "..." }`.
fn reasoning_text_delta(reasoning_val: &Value) -> Option<String> {
    if let Some(s) = reasoning_val.as_str().filter(|s| !s.is_empty()) {
        return Some(s.to_string());
    }
    if reasoning_val.is_object() {
        if let Some(s) = reasoning_val
            .get("text")
            .and_then(|t| t.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(s.to_string());
        }
        if let Some(s) = reasoning_val
            .get("content")
            .and_then(|t| t.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(s.to_string());
        }
    }
    None
}

/// Extract reasoning text from a `choice.message.reasoning` field, accepting
/// either a plain string or an object with `{ text | content }`.
fn reasoning_text_from_value(message_reasoning: &Value) -> Option<String> {
    if let Some(s) = message_reasoning.as_str() {
        return Some(s.to_string());
    }
    if let Some(obj) = message_reasoning.as_object() {
        if let Some(s) = obj
            .get("text")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("content").and_then(|v| v.as_str()))
        {
            return Some(s.to_string());
        }
    }
    None
}
