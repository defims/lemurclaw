//! Chat Completions tool-JSON helper, ported (in extract) from
//! just-every/code's `code-rs/core/src/openai_tools.rs`.
//!
//! code-rs's `openai_tools.rs` is a 2981-line module covering tool construction
//! for both the Responses and Chat Completions APIs, MCP tool conversion, JSON
//! Schema sanitization, etc. codex-rs already carries the Responses-API side of
//! this surface area in the dedicated `codex_tools` crate
//! (`create_tools_json_for_responses_api`, `ToolSpec`, `ResponsesApiTool`, ...),
//! so the only piece we need to re-enable `wire_api = "chat"` is the thin
//! adapter that rewrites Responses-API tool JSON into the Chat Completions
//! `{"type":"function","function":{...}}` shape. That is all this file ports.

use codex_tools::create_tools_json_for_responses_api;
use codex_tools::ToolSpec;
use serde_json::Value;
use serde_json::json;

/// Returns JSON values that are compatible with Function Calling in the
/// Chat Completions API:
/// <https://platform.openai.com/docs/guides/function-calling?api-mode=chat>
///
/// This mirrors `create_tools_json_for_chat_completions_api` from code-rs's
/// `openai_tools.rs` (lines ~1200-1226). It starts from the Responses-API tool
/// JSON (produced by `codex_tools::create_tools_json_for_responses_api`) and
/// rewrites each `function` tool so it is nested under a `function` key with
/// `type: "function"` at the top level, which is the shape the Chat Completions
/// API expects. Non-`function` tools (e.g. `web_search`, `tool_search`,
/// `namespace`) are dropped because they are Responses-API-only and have no
/// Chat Completions equivalent.
pub(crate) fn create_tools_json_for_chat_completions_api(
    tools: &[ToolSpec],
) -> Result<Vec<Value>, serde_json::Error> {
    let responses_api_tools_json = create_tools_json_for_responses_api(tools)?;
    let tools_json = responses_api_tools_json
        .into_iter()
        .filter_map(|mut tool| {
            // The `ToolSpec` enum serializes with `#[serde(tag = "type")]`, so a
            // `Function` variant becomes `{"type":"function", ...}`. We only
            // propagate function tools; everything else is unsupported by Chat
            // Completions.
            if tool.get("type") != Some(&Value::String("function".to_string())) {
                return None;
            }

            if let Some(map) = tool.as_object_mut() {
                // Remove the outer "type" field; it is re-introduced at the top
                // level below and is not part of the nested `function` object.
                map.remove("type");
                Some(json!({
                    "type": "function",
                    "function": map,
                }))
            } else {
                None
            }
        })
        .collect::<Vec<Value>>();
    Ok(tools_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_tools::JsonSchema;
    use codex_tools::ResponsesApiTool;

    #[test]
    fn function_tool_is_nested_under_function_key() {
        let tool = ToolSpec::Function(ResponsesApiTool {
            name: "echo".to_string(),
            description: "echoes back".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::default(),
            output_schema: None,
        });
        let json = create_tools_json_for_chat_completions_api(&[tool]).unwrap();
        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["type"], "function");
        assert_eq!(json[0]["function"]["name"], "echo");
        // The nested function object must not carry its own `type` field.
        assert!(json[0]["function"].get("type").is_none());
    }
}
