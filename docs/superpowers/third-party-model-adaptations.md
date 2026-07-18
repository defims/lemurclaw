# Third-Party Model Chat Completions — Adaptation Record

This document records every codex-rs vs code-rs difference that was resolved
while porting third-party Chat Completions model support from
`just-every/code` (`code-rs/core/src/`) into this lemurclaw fork of
`openai/codex` (`codex-rs/core/`). It is the artifact required by Task 1.3
Step 7 of the lemurclaw plan.

Source files ported from (reference):
- `code-rs/core/src/chat_completions.rs` (1471 lines)
- `code-rs/core/src/model_family.rs` (595 lines)
- `code-rs/core/src/openai_tools.rs` (2981 lines — only the chat helper extracted)
- `code-rs/core/src/client.rs` (reference for the Chat arm pattern, lines 612-707)

Files written in the fork:
- `codex-rs/core/src/model_family.rs`
- `codex-rs/core/src/chat_completions.rs`
- `codex-rs/core/src/openai_tools.rs`
- `codex-rs/core/src/client.rs` (Chat arm added to the `match wire_api`)
- `codex-rs/codex-api/src/endpoint/chat_completions.rs` (new)
- `codex-rs/codex-api/src/sse/chat_completions.rs` (new)

## Architectural difference (the big one)

code-rs builds HTTP requests for Chat Completions directly with `reqwest`
inside `chat_completions.rs`, treating `ModelProviderInfo` as a request
builder with methods like `effective_auth`, `create_request_builder_with_auth`,
`get_full_url`, `is_public_openai_chat_endpoint`, `openrouter_config`, etc.

codex-rs organizes transport/auth behind a layered abstraction in the
`codex-api` and `codex-client` crates: `Provider` + `SharedAuthProvider` +
`HttpTransport` (`ReqwestTransport`) + `EndpointSession`, with
per-endpoint clients (`ResponsesClient`, `CompactClient`, ...) and a shared
SSE processor (`sse::responses`). The `ModelProviderInfo` in
`codex-rs/model-provider-info` is a pure config struct with **none** of the
request-building methods code-rs relies on.

Resolution: rather than porting code-rs's reqwest-direct flow (which would
bypass codex-rs's auth/retry/telemetry plumbing), the Chat Completions path
was implemented as a first-class endpoint client that mirrors
`ResponsesClient`:

- `codex-api/src/endpoint/chat_completions.rs` — `ChatCompletionsClient`
  (reuses `EndpointSession`, so auth headers, query params, retry policy,
  and request telemetry all keep working for third-party providers).
- `codex-api/src/sse/chat_completions.rs` — Chat-Completions-specific SSE
  processor that emits codex-native `ResponseEvent`s.
- `core/src/chat_completions.rs` — only the message-translation logic
  (`ResponseItem` history → Chat Completions `messages` array).
- `core/src/client.rs` — `stream_chat_completions` wires the above together
  using the same `current_client_setup()` + `build_api_transport()` flow as
  `stream_responses_api`, then bridges `codex_api::ResponseStream` → core
  `ResponseStream` via `bridge_chat_completions_stream`.

## Type / module mapping

### `core/src/model_family.rs`

| code-rs import / usage | codex-rs resolution |
|---|---|
| `crate::config_types::Personality` | `codex_protocol::config_types::Personality` |
| `crate::config_types::ContextMode` | **Does not exist in codex-rs.** `ContextMode` (OneM/Auto/Disabled) is a code-rs-only feature. The `resolve_context_mode_limits` helper that consumed it was **removed**; `supports_extended_context` + `EXTENDED_CONTEXT_WINDOW_1M` + `default_auto_compact_limit_for_context_window` are retained (unused for now) so a future fork can re-add the helper after porting `ContextMode`. |
| `crate::config_types::ReasoningSummary` | `codex_protocol::config_types::ReasoningSummary` |
| `crate::openai_models::ReasoningEffort` | `codex_protocol::openai_models::ReasoningEffort` |
| `crate::tool_apply_patch::ApplyPatchToolType` | `codex_protocol::openai_models::ApplyPatchToolType` (codex-rs has no `tool_apply_patch` module; the type lives directly in `openai_models`). |
| `code_protocol::models::*` | `codex_protocol::models::*` (and `openai_models::*`) |

### `core/src/model_family.rs` — `ModelInfo` field differences

codex-rs's `ModelInfo` (`protocol/src/openai_models.rs`) differs from
code-rs's in two spots that `apply_upstream_model_overrides` touches:

| code-rs `ModelInfo` field | codex-rs resolution |
|---|---|
| `supports_reasoning_summaries: bool` | Renamed to `supports_reasoning_summary_parameter` in codex-rs. Mapped accordingly. |
| `prefer_websockets: bool` | **Absent** in codex-rs's `ModelInfo`. `ModelFamily.prefer_websockets` is hard-coded to `false` (HTTP transport), matching codex-rs's default. The test assertion that checked it was dropped with a documenting comment. |
| `ApplyPatchToolType::Function` variant | **Absent** in codex-rs (only `Freeform`). The `From<&model_info::ApplyPatchToolType>` match now has a single `Freeform => Freeform` arm; the `gpt-oss` family literal that used `Function` was changed to `Freeform`. |

### `core/src/model_family.rs` — `ReasoningEffort` variant difference

codex-rs's `ReasoningEffort` has an `Ultra` variant that code-rs's does
not. The default-effort mapping (`apply_upstream_model_overrides`) gained
an `Ultra => Ultra` arm. (The other variants already matched; `Max` still
collapses to `XHigh` to preserve code-rs intent.)

### `core/src/model_family.rs` — missing `include_str!` prompt files

Two `include_str!` targets that exist in code-rs were missing from
`codex-rs/core/`:

- `prompt.md` — copied verbatim from `code-rs/core/prompt.md` (byte-identical
  to the existing `codex-rs/models-manager/prompt.md`; the new copy sits at
  `codex-rs/core/prompt.md` to match the `include_str!("../prompt.md")`
  path).
- `gpt-5.2-codex_prompt.md` — copied from `code-rs/core/`.

(The other prompt files — `gpt_5_codex_prompt.md`, `gpt_5_1_prompt.md`,
`gpt_5_2_prompt.md`, `gpt-5.1-codex-max_prompt.md`,
`prompt_with_apply_patch_instructions.md` — already existed in codex-rs.)

### `core/src/chat_completions.rs`

| code-rs import / concept | codex-rs resolution |
|---|---|
| `crate::auth::AuthManager` | **No `auth` module in core.** Auth is handled by `codex_login::AuthManager` + `codex_api::SharedAuthProvider`, applied inside `EndpointSession`. The Chat Completions body builder no longer takes or uses an `AuthManager`. |
| `crate::ModelProviderInfo` | `codex_model_provider_info::ModelProviderInfo`, but the body builder doesn't need it — only the client (`ChatCompletionsClient`) does, and it takes `codex_api::Provider` instead. |
| `crate::debug_logger::DebugLogger` | **No `debug_logger` module in core.** code-rs's per-request debug logging has no codex-rs equivalent. The body builder no longer interacts with a debug logger; transport-level tracing (`tracing::debug!`/`trace!` inside `ReqwestTransport` and the SSE processor) covers diagnostics. |
| `crate::error::{CodexErr, Result, RetryLimitReachedError, UnexpectedResponseError}` | `codex_protocol::error::{CodexErr, Result, ...}`. The Chat Completions body builder returns `Result<_, serde_json::Error>`; the retry/error mapping happens at the `codex-api` layer (`map_api_error`) and in `bridge_chat_completions_stream`. |
| `crate::client_common::{replace_image_payloads_for_model, rewrite_image_generation_calls_for_input}` | **Code-rs-only helpers.** Not ported: codex-rs already has `Prompt::get_formatted_input_for_request` which is the equivalent input-preparation entry point, and image handling is done earlier in the pipeline. |
| `crate::client_common::Prompt::get_full_instructions` / `get_formatted_input` | **Not present on codex-rs's `Prompt`.** Replaced with `prompt.base_instructions.text` (system message) and `prompt.get_formatted_input_for_request(false)` (history), matching how `build_responses_request` populates the Responses-API `instructions`/`input` fields. |
| `code_otel::otel_event_manager::OtelEventManager` | **No `code_otel` crate.** codex-rs uses `codex_otel` for telemetry. SSE telemetry is plumbed through `codex_api::SseTelemetry` (used by `ChatCompletionsClient::with_telemetry`); the SSE idle-timeout loop mirrors `process_sse_with_treatment`'s telemetry hook. |
| `crate::util::backoff` | Not needed in the body builder. codex-rs handles retries inside `EndpointSession` via `Provider.retry.to_policy()` + `run_with_request_telemetry`, not with a manual `backoff()` loop. |

### `core/src/chat_completions.rs` — `ResponseItem` variant differences

code-rs's `ResponseItem` has `CompactionSummary` and `GhostSnapshot`
variants; codex-rs uses `Compaction`, `CompactionTrigger`,
`ContextCompaction`, `AgentMessage`, and `Other` instead. The message
translation's match was rewritten against codex-rs's actual variants:

- `Compaction` / `CompactionTrigger` / `ContextCompaction` / `AgentMessage`
  are dropped (no Chat Completions equivalent, same as code-rs's
  `CompactionSummary`).
- `Other` is dropped (matches code-rs).

### `core/src/chat_completions.rs` — `ResponseItem::Message` field difference

code-rs's `Message` carries an `end_turn: Option<bool>` field; codex-rs's
does not. The struct-literal assembly of translated messages doesn't
construct `Message` items (it builds raw JSON), so this only affected the
SSE processor (see below).

### `core/src/openai_tools.rs`

| code-rs concept | codex-rs resolution |
|---|---|
| `crate::error::Result` | `Result<_, serde_json::Error>` (matches codex-rs's existing `codex_tools::create_tools_json_for_responses_api` signature). |
| `OpenAiTool` enum | `codex_tools::ToolSpec` (codex-rs's equivalent, already the type stored on `Prompt.tools`). |
| `create_tools_json_for_responses_api` (code-rs local fn) | `codex_tools::create_tools_json_for_responses_api` (already exported by the `codex-tools` crate). |

Only `create_tools_json_for_chat_completions_api` was ported — it is a pure
post-processing adapter over the Responses-API JSON, so it needed no
further changes beyond the input type rename. The other ~2900 lines of
code-rs's `openai_tools.rs` (MCP tool conversion, JSON Schema
sanitization, shell/search/image tool constructors) already live in
codex-rs's `codex-tools` crate.

### `codex-api/src/sse/chat_completions.rs` — `ResponseEvent` shape differences

code-rs's `ResponseEvent` variants are structur with extra fields:

| code-rs `ResponseEvent` variant | codex-rs variant |
|---|---|
| `OutputTextDelta { delta, item_id, sequence_number, output_index }` | `OutputTextDelta(String)` — only the delta string is carried; `item_id`/`sequence_number`/`output_index` are dropped. |
| `ReasoningContentDelta { delta, item_id, sequence_number, output_index, content_index }` | `ReasoningContentDelta { delta, content_index }` — only `delta` + `content_index` (hard-coded `0`). |
| `Completed { response_id, token_usage }` | `Completed { response_id, token_usage, end_turn }` — codex-rs additionally carries `end_turn: Option<bool>`. |
| `Created { response_id, response_model }` | `Created` (no fields). |
| `OutputItemDone { item, sequence_number, output_index }` | `OutputItemDone(ResponseItem)` — only the item. |

The SSE processor was rewritten to emit codex-rs's variants.

### `codex-api/src/sse/chat_completions.rs` — `ResponseItem::Message` / `Reasoning` field differences

codex-rs's `Message` has no `end_turn` field (it has `phase` +
`internal_chat_message_metadata_passthrough` instead), and `Reasoning` /
`FunctionCall` carry an `internal_chat_message_metadata_passthrough` field.
All struct literals were assembled against codex-rs's field set, with
`internal_chat_message_metadata_passthrough: None` and `phase: None`.

### `codex-api/src/sse/chat_completions.rs` — `ResponseItemId` newtype

codex-rs's `ResponseItem` id fields are `Option<ResponseItemId>` (a
newtype wrapper around `String`), not `Option<String>` as in code-rs.
A small `item_id_from(&Option<String>) -> Option<ResponseItemId>` helper
converts the server-provided id string via `ResponseItemId::from_server`,
filtering empty strings.

### `core/src/client.rs` — `WireApi` dispatch

code-rs's `client.rs` carries a 3-arm `match wire_api` (`Responses`,
`Chat`, `ResponsesWebsocket`). codex-rs's `client.rs` (post the #7782
removal that this fork already reverted) had only the `Responses` arm.
Added the `Chat` and `ResponsesWebsocket` arms:

- `WireApi::Chat` calls the new `ModelClientSession::stream_chat_completions`,
  which builds the payload via `crate::chat_completions`, optionally
  sanitizes message roles for non-OpenAI providers, then drives
  `ChatCompletionsClient::stream` through the standard
  `current_client_setup()` + `build_api_transport()` flow.
- `WireApi::ResponsesWebsocket` returns
  `CodexErr::UnsupportedOperation(...)` (stub — out of scope for this task).

A new `bridge_chat_completions_stream` free function converts the
`codex_api::ResponseStream` into a core `ResponseStream` (the two types
differ: core's adds a `consumer_dropped: CancellationToken`). It forwards
events, maps `ApiError → CodexErr` via `codex_api::map_api_error`, and
honours the consumer-dropped cancellation token. This bridge is lighter
than the Responses path's `map_response_stream` (which also drives
`InferenceTraceAttempt` + `LastResponse` tracking) — wiring the Chat path
into those is left as future work.

## What was intentionally NOT ported

- `code-rs`'s OpenRouter provider config (`provider.openrouter_config()`),
  Ollama `num_ctx` env override, and `is_public_openai_chat_endpoint()`
  branching in `stream_chat_completions`. These are code-rs-specific
  provider-quality-of-life features that have no codex-rs equivalent on
  `ModelProviderInfo`. They can be added later by extending
  `ModelProviderInfo` if needed.
- code-rs's `AggregatedChatStream` / `AggregateStreamExt` (the optional
  client-side aggregation adapter). codex-rs's session loop does not use
  that pattern; the Chat path emits the standard delta + terminal-item
  event sequence that the rest of the pipeline expects.
- code-rs's per-request `DebugLogger` call sites (no equivalent in
  codex-rs; `tracing` covers diagnostics).
- The manual `backoff()` retry loop. codex-rs's
  `EndpointSession::stream_encoded_json_with` already applies the
  provider retry policy via `run_with_request_telemetry`.
