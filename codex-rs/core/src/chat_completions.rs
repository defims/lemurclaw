//! Chat Completions request body builder.
//!
//! This module translates codex's internal conversation representation
//! ([`codex_protocol::models::ResponseItem`]) into the `messages` array shape
//! expected by the OpenAI Chat Completions API
//! (<https://platform.openai.com/docs/api-reference/chat>), and combines it
//! with the Chat Completions tool JSON produced by [`crate::openai_tools`].
//!
//! The translation logic is adapted from just-every/code's
//! `code-rs/core/src/chat_completions.rs` (`stream_chat_completions`,
//! specifically the message-building section). The HTTP transport, SSE
//! parsing, and auth wiring live in `codex-api`
//! (`ChatCompletionsClient` + `sse::chat_completions`), mirroring the way the
//! Responses API path is structured. This split keeps the core free of
//! transport concerns.
//!
//! Key adaptations from code-rs (see
//! `docs/superpowers/third-party-model-adaptations.md` for the full record):
//! - code-rs's `ResponseItem` has `CompactionSummary` / `GhostSnapshot`
//!   variants; codex-rs uses `Compaction` / `CompactionTrigger` /
//!   `ContextCompaction` / `Other` instead. We drop all compaction-related and
//!   non-Chat-Completions-relevant items.
//! - code-rs's `ResponseItem::Message` carries an `end_turn` field; codex-rs's
//!   does not.

use crate::client_common::Prompt;
use crate::openai_tools::create_tools_json_for_chat_completions_api;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use serde_json::Value;
use serde_json::json;

/// Build a complete Chat Completions API request payload from a codex
/// [`Prompt`] and the resolved model slug.
///
/// The returned JSON value is ready to be sent as the body of a
/// `POST {provider_base_url}/chat/completions` request. It contains:
/// - `model`: the resolved model slug
/// - `messages`: a system message with the prompt's base instructions
///   followed by the conversation history translated into Chat Completions
///   message objects
/// - `tools`: the Chat Completions tool array (only `function` tools; see
///   [`crate::openai_tools`])
/// - `stream: true` and `store: false`
///
/// The instructions are sourced from `prompt.base_instructions.text`, matching
/// how the Responses API path (`build_responses_request`) populates its
/// `instructions` field. This keeps the two wire formats consistent for a
/// given thread.
pub(crate) fn build_chat_completions_payload(
    prompt: &Prompt,
    model_slug: &str,
) -> Result<Value, serde_json::Error> {
    let mut messages = Vec::<Value>::new();

    // System message: the prompt's base instructions. The Responses API sends
    // this separately as `instructions`; Chat Completions has no such field,
    // so we inline it as the first `system` message. Only emit it when
    // non-empty so we don't send a contentless system message to strict
    // providers.
    if !prompt.base_instructions.text.is_empty() {
        messages.push(json!({
            "role": "system",
            "content": prompt.base_instructions.text.clone(),
        }));
    }

    let input = prompt.get_formatted_input_for_request(false);
    let reasoning_by_anchor_index = collect_reasoning_anchors(&input);

    for (idx, item) in input.iter().enumerate() {
        match item {
            ResponseItem::Message { role, content, .. } => {
                push_message(&mut messages, role, content);
            }
            ResponseItem::AgentMessage { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::CompactionTrigger { .. }
            | ResponseItem::ContextCompaction { .. } => {
                // These items are Responses-API-internal or codex-local and
                // have no Chat Completions equivalent; omit them.
                continue;
            }
            ResponseItem::FunctionCall {
                name,
                arguments,
                call_id,
                ..
            } => {
                let reasoning = reasoning_by_anchor_index.get(&idx).map(String::as_str);
                let tool_call = json!({
                    "id": call_id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": arguments,
                    }
                });
                push_tool_call_message(&mut messages, tool_call, reasoning);
            }
            ResponseItem::ToolSearchCall {
                call_id,
                status,
                execution,
                arguments,
                ..
            } => {
                let reasoning = reasoning_by_anchor_index.get(&idx).map(String::as_str);
                let tool_call = json!({
                    "id": call_id.clone().unwrap_or_default(),
                    "type": "tool_search_call",
                    "call_id": call_id,
                    "status": status,
                    "execution": execution,
                    "arguments": arguments,
                });
                push_tool_call_message(&mut messages, tool_call, reasoning);
            }
            ResponseItem::LocalShellCall {
                id,
                call_id: _,
                status,
                action,
                ..
            } => {
                let reasoning = reasoning_by_anchor_index.get(&idx).map(String::as_str);
                let tool_call = json!({
                    "id": id.as_ref().map(AsRef::<str>::as_ref).unwrap_or_default(),
                    "type": "local_shell_call",
                    "status": status,
                    "action": action,
                });
                push_tool_call_message(&mut messages, tool_call, reasoning);
            }
            ResponseItem::FunctionCallOutput { call_id, output, .. } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": output.to_string(),
                }));
            }
            ResponseItem::ToolSearchOutput {
                call_id,
                status,
                execution,
                tools,
                ..
            } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id.clone().unwrap_or_default(),
                    "content": serde_json::json!({
                        "status": status,
                        "execution": execution,
                        "tools": tools,
                    })
                    .to_string(),
                }));
            }
            ResponseItem::CustomToolCall {
                id,
                call_id: _,
                name,
                namespace: _,
                input,
                status: _,
                ..
            } => {
                let reasoning = reasoning_by_anchor_index.get(&idx).map(String::as_str);
                let tool_call = json!({
                    "id": id.as_ref().map(AsRef::<str>::as_ref).unwrap_or_default(),
                    "type": "custom",
                    "custom": {
                        "name": name,
                        "input": input,
                    }
                });
                push_tool_call_message(&mut messages, tool_call, reasoning);
            }
            ResponseItem::CustomToolCallOutput {
                call_id, output, ..
            } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": output.to_string(),
                }));
            }
            ResponseItem::AdditionalTools { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Other => {
                // Omit these items from the conversation history.
                continue;
            }
        }
    }

    let tools_json = create_tools_json_for_chat_completions_api(&prompt.tools)?;
    Ok(create_chat_completions_payload(model_slug, messages, tools_json))
}

/// Pre-scan the input and map each `Reasoning` block (that appears after the
/// last user message and not at the tail of the conversation) onto the
/// adjacent assistant anchor — either the previous assistant message (stop
/// turns) or the next assistant-shaped anchor (tool-call turns).
///
/// This preserves reasoning context for providers that support a `reasoning`
/// field on assistant messages, while dropping reasoning that would otherwise
/// dangle without an anchor.
fn collect_reasoning_anchors(
    input: &[ResponseItem],
) -> std::collections::HashMap<usize, String> {
    let mut anchors: std::collections::HashMap<usize, String> =
        std::collections::HashMap::new();

    // Determine the last role that would be emitted to Chat Completions.
    let mut last_emitted_role: Option<&str> = None;
    for item in input {
        match item {
            ResponseItem::Message { role, .. } => last_emitted_role = Some(role.as_str()),
            ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::LocalShellCall { .. } => last_emitted_role = Some("assistant"),
            ResponseItem::FunctionCallOutput { .. } | ResponseItem::ToolSearchOutput { .. } => {
                last_emitted_role = Some("tool")
            }
            _ => {}
        }
    }

    // Find the last user message index in the input.
    let mut last_user_index: Option<usize> = None;
    for (idx, item) in input.iter().enumerate() {
        if let ResponseItem::Message { role, .. } = item {
            if role == "user" {
                last_user_index = Some(idx);
            }
        }
    }

    // Attach reasoning only if the conversation does not end with a user message.
    if matches!(last_emitted_role, Some("user")) {
        return anchors;
    }

    for (idx, item) in input.iter().enumerate() {
        if let Some(u_idx) = last_user_index {
            if idx <= u_idx {
                continue;
            }
        }

        let ResponseItem::Reasoning {
            content: Some(items),
            ..
        } = item
        else {
            continue;
        };

        let mut text = String::new();
        for c in items {
            match c {
                ReasoningItemContent::ReasoningText { text: t }
                | ReasoningItemContent::Text { text: t } => text.push_str(t),
            }
        }
        if text.trim().is_empty() {
            continue;
        }

        // Prefer immediate previous assistant message (stop turns).
        let mut attached = false;
        if idx > 0 {
            if let ResponseItem::Message { role, .. } = &input[idx - 1] {
                if role == "assistant" {
                    anchors
                        .entry(idx - 1)
                        .and_modify(|v| v.push_str(&text))
                        .or_insert_with(|| text.clone());
                    attached = true;
                }
            }
        }

        // Otherwise, attach to immediate next assistant anchor.
        if !attached && idx + 1 < input.len() {
            match &input[idx + 1] {
                ResponseItem::FunctionCall { .. }
                | ResponseItem::ToolSearchCall { .. }
                | ResponseItem::LocalShellCall { .. } => {
                    anchors
                        .entry(idx + 1)
                        .and_modify(|v| v.push_str(&text))
                        .or_insert_with(|| text.clone());
                }
                ResponseItem::Message { role, .. } if role == "assistant" => {
                    anchors
                        .entry(idx + 1)
                        .and_modify(|v| v.push_str(&text))
                        .or_insert_with(|| text.clone());
                }
                _ => {}
            }
        }
    }

    anchors
}

/// Push a Chat Completions message for a codex `Message` item, choosing the
/// multimodal array form when the message contains images and the plain-string
/// form otherwise.
fn push_message(messages: &mut Vec<Value>, role: &str, content: &[ContentItem]) {
    let contains_image = content
        .iter()
        .any(|c| matches!(c, ContentItem::InputImage { .. }));

    if contains_image {
        let mut parts = Vec::<Value>::new();
        for c in content {
            match c {
                ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                    parts.push(json!({ "type": "text", "text": text }));
                }
                ContentItem::InputImage { image_url, .. } => {
                    parts.push(json!({
                        "type": "image_url",
                        "image_url": { "url": image_url }
                    }));
                }
                ContentItem::InputAudio { audio_url } => {
                    parts.push(json!({
                        "type": "input_audio",
                        "input_audio": { "data": audio_url }
                    }));
                }
            }
        }
        messages.push(json!({ "role": role, "content": parts }));
    } else {
        // Text-only messages can be sent as a single string for maximal
        // compatibility with providers that only accept plain text.
        let mut text = String::new();
        for c in content {
            match c {
                ContentItem::InputText { text: t } | ContentItem::OutputText { text: t } => {
                    text.push_str(t);
                }
                _ => {}
            }
        }
        messages.push(json!({ "role": role, "content": text }));
    }
}

/// Chat Completions requires that tool calls are grouped into a single
/// assistant message (with `tool_calls: [...]`) followed by `role: "tool"`
/// responses. This helper either appends to the trailing assistant tool-call
/// message or starts a new one.
fn push_tool_call_message(messages: &mut Vec<Value>, tool_call: Value, reasoning: Option<&str>) {
    if let Some(Value::Object(obj)) = messages.last_mut()
        && obj.get("role").and_then(Value::as_str) == Some("assistant")
        && obj.get("content").is_some_and(Value::is_null)
        && let Some(tool_calls) = obj.get_mut("tool_calls").and_then(Value::as_array_mut)
    {
        tool_calls.push(tool_call);
        if let Some(reasoning) = reasoning {
            if let Some(Value::String(existing)) = obj.get_mut("reasoning") {
                if !existing.is_empty() {
                    existing.push('\n');
                }
                existing.push_str(reasoning);
            } else {
                obj.insert("reasoning".to_string(), Value::String(reasoning.to_string()));
            }
        }
        return;
    }

    let mut msg = json!({
        "role": "assistant",
        "content": Value::Null,
        "tool_calls": [tool_call],
    });
    if let Some(reasoning) = reasoning
        && let Some(obj) = msg.as_object_mut()
    {
        obj.insert("reasoning".to_string(), json!(reasoning));
    }
    messages.push(msg);
}

fn create_chat_completions_payload(
    model_slug: &str,
    messages: Vec<Value>,
    tools_json: Vec<Value>,
) -> Value {
    json!({
        "model": model_slug,
        "messages": messages,
        "stream": true,
        "store": false,
        "tools": tools_json,
    })
}

/// Normalize outgoing Chat Completions message roles to the string role set
/// accepted by strict OpenAI-compatible providers (`system`, `user`,
/// `assistant`, `tool`).
///
/// OpenAI Chat Completions accepts `developer`, but many self-hosted or
/// third-party gateways validate the role against a strict serde enum and
/// return a 400 for unknown string variants. Custom string roles are rewritten
/// to `system`, which preserves the instructional intent while leaving message
/// `content` untouched. Missing or non-string roles are left unchanged so
/// malformed payloads fail visibly instead of being silently repaired.
pub(crate) fn sanitize_message_roles_for_strict_chat_providers(payload: &mut Value) {
    let Some(messages) = payload.get_mut("messages").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for message in messages.iter_mut() {
        let Some(obj) = message.as_object_mut() else {
            continue;
        };
        match obj.get("role").and_then(|r| r.as_str()) {
            Some("system" | "user" | "assistant" | "tool") | None => {}
            Some(_) => {
                obj.insert(
                    "role".to_string(),
                    Value::String("system".to_string()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::create_chat_completions_payload;
    use super::sanitize_message_roles_for_strict_chat_providers;
    use serde_json::json;

    #[test]
    fn chat_completions_payload_disables_openai_storage() {
        let payload = create_chat_completions_payload(
            "gpt-4.1",
            vec![json!({"role": "user", "content": "what is 2+2?"})],
            Vec::new(),
        );

        assert_eq!(payload["store"], false);

        let body = serde_json::to_string(&payload).unwrap();
        assert!(
            body.contains("\"store\":false"),
            "serialized payload should explicitly disable storage: {body}"
        );
    }

    #[test]
    fn normalizes_developer_role_to_system_before_strict_provider_serialization() {
        let mut payload = json!({
            "model": "kimi-k2",
            "messages": [
                {"role": "developer", "content": "instructions"},
                {"role": "user", "content": "hello"},
            ],
        });

        sanitize_message_roles_for_strict_chat_providers(&mut payload);

        let messages = payload["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "instructions");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "hello");

        let body = serde_json::to_string(&payload).unwrap();
        assert!(
            !body.contains("\"role\":\"developer\""),
            "serialized payload still contains the rejected role: {body}"
        );
    }

    #[test]
    fn rewrites_unknown_string_roles_but_keeps_standard_ones() {
        let mut payload = json!({
            "messages": [
                {"role": "system", "content": "sys"},
                {"role": "user", "content": "u"},
                {"role": "assistant", "content": "a"},
                {"role": "tool", "tool_call_id": "1", "content": "t"},
                {"role": "agent", "content": "custom"},
            ],
        });

        sanitize_message_roles_for_strict_chat_providers(&mut payload);

        let roles: Vec<&str> = payload["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["role"].as_str().expect("role should be a string"))
            .collect();
        assert_eq!(
            roles,
            vec!["system", "user", "assistant", "tool", "system"]
        );
    }

    #[test]
    fn payload_without_messages_is_left_unchanged() {
        let mut payload = json!({"model": "x"});
        sanitize_message_roles_for_strict_chat_providers(&mut payload);
        assert_eq!(payload, json!({"model": "x"}));
    }

    #[test]
    fn malformed_roles_are_left_unchanged() {
        let mut payload = json!({
            "messages": [
                {"role": 42, "content": "numeric role"},
                {"content": "missing role"},
            ],
        });

        sanitize_message_roles_for_strict_chat_providers(&mut payload);

        assert_eq!(
            payload,
            json!({
                "messages": [
                    {"role": 42, "content": "numeric role"},
                    {"content": "missing role"},
                ],
            })
        );
    }
}
