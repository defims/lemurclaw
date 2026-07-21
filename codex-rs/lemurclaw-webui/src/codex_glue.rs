//! Shared codex-protocol glue used by both the wry GUI (`lemurclaw-gui`) and
//! the in-process webui server (`server.rs`).
//!
//! Everything here is pure codex JSON-RPC plumbing — no wry/tao, no axum, no
//! UI. Lifted out of `lemurclaw-gui/src/backend.rs` so the WS bridge and the
//! wry IPC handler build the same wire shapes identically, and the dev
//! `InProcessAppServerClient` bootstrap is not duplicated.
//!
//! Wire contract preserved verbatim:
//! - JS → Rust (upstream): `{"__resolve": id, "result": {...}}`,
//!   `{"__reject": id, "error": {code, message}}`, or any
//!   `ClientRequest`-shaped JSON object.
//! - Rust → JS (downstream): raw `ServerNotification`/`ServerRequest` JSON for
//!   pushed events, and `{jsonrpc:"2.0", id, result|error}` envelopes for
//!   per-ClientRequest responses (matched by id in transport.ts /
//!   transport-ws.ts).

use std::sync::Arc;

use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::EnvironmentManager;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessAppServerRequestHandle;
use codex_app_server_client::InProcessClientStartArgs;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_client::legacy_core::config::ConfigBuilder;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result as JsonRpcResult;
use codex_arg0::Arg0DispatchPaths;
use codex_config::LoaderOverrides;
use codex_protocol::protocol::SessionSource;
use codex_rollout::state_db;

/// Distinguishes the two envelope kinds handled in `handle_ipc` /
/// `handle_ipc_async`.
pub enum ResolveKind {
    Resolve,
    Reject,
}

/// The outcome of a `ClientRequest` as returned by
/// [`InProcessAppServerRequestHandle::request`]. Mirrors the codex type so we
/// can take it by value in [`build_response_envelope`] without naming the
/// codex crate's alias at every call site.
pub type RequestResult = Result<JsonRpcResult, JSONRPCErrorError>;

/// Respond to (resolve or reject) a pending ServerRequest on the backend
/// runtime. Awaited inside the task spawned by `handle_ipc` / `handle_ipc`
/// (does not spawn anything itself). Pulls the RequestId / JSONRPCErrorError
/// types from the protocol crate; falls back to a safe null/error payload on
/// malformed input so a bad JS envelope never kills the worker.
pub async fn respond_to_server_request(
    handle: &InProcessAppServerRequestHandle,
    request_id: serde_json::Value,
    payload: serde_json::Value,
    kind: ResolveKind,
) {
    let req_id = match serde_json::from_value::<RequestId>(request_id) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[lemurclaw] resolve: bad request_id: {e}");
            return;
        }
    };
    let result = match kind {
        ResolveKind::Resolve => {
            // `codex_app_server_protocol::Result` is a type alias for
            // `serde_json::Value` (the JSON-RPC result payload). On a
            // malformed envelope fall back to JSON null rather than failing
            // the resolve, so a bad JS body never hangs an approval flow.
            let json_result: JsonRpcResult =
                serde_json::from_value(payload).unwrap_or(serde_json::Value::Null);
            handle.resolve_server_request(req_id, json_result).await
        }
        ResolveKind::Reject => {
            let err: JSONRPCErrorError =
                serde_json::from_value(payload).unwrap_or_else(|e| JSONRPCErrorError {
                    code: -32000,
                    message: format!("malformed reject payload: {e}"),
                    data: None,
                });
            handle.reject_server_request(req_id, err).await
        }
    };
    if let Err(e) = result {
        eprintln!("[lemurclaw] resolve/reject failed: {e}");
    }
}

/// Build the JSON-RPC `{jsonrpc, id, result|error}` envelope for a
/// `ClientRequest` response. `req_id_json` is the raw `id` captured from the
/// inbound request Value (preserves string vs number shape without having to
/// dig it back out of the deserialized `ClientRequest` enum).
///
/// For the error arm the `data` field is omitted entirely when `err.data` is
/// `None`, honoring the protocol's `skip_serializing_if = "Option::is_none"`
/// (vs. a naive `json!({"data": err.data})` which always emits `"data": null`).
pub fn build_response_envelope(
    req_id_json: serde_json::Value,
    result: RequestResult,
) -> serde_json::Value {
    match result {
        Ok(val) => serde_json::json!({
            "jsonrpc": "2.0", "id": req_id_json, "result": val,
        }),
        Err(err) => {
            let mut error_obj = serde_json::Map::new();
            error_obj.insert("code".into(), err.code.into());
            error_obj.insert("message".into(), err.message.into());
            if let Some(data) = err.data {
                error_obj.insert("data".into(), data);
            }
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": req_id_json,
                "error": serde_json::Value::Object(error_obj),
            })
        }
    }
}

/// Serialize an `InProcessServerEvent` for JS delivery.
///
/// `InProcessServerEvent` itself does not derive `Serialize`, but its inner
/// `ServerNotification` / `ServerRequest` variants do. The `Lagged` variant
/// (which carries only a `skipped` count) gets a hand-rolled JSON envelope so
/// the JS side still sees *something* useful instead of a silent drop.
pub fn serialize_server_event(event: InProcessServerEvent) -> String {
    match event {
        InProcessServerEvent::ServerNotification(n) => serde_json::to_string(&n)
            .unwrap_or_else(|e| format!(r#"{{"serializeError":"notification: {e}"}}"#)),
        InProcessServerEvent::ServerRequest(r) => serde_json::to_string(&r)
            .unwrap_or_else(|e| format!(r#"{{"serializeError":"serverRequest: {e}"}}"#)),
        InProcessServerEvent::Lagged { skipped } => {
            format!(r#"{{"type":"lagged","skipped":{skipped}}}"#)
        }
    }
}

/// Build the in-process AppServerClient using the dev path (no real auth,
/// `EnvironmentManager::default_for_tests()`). This is sufficient for
/// subproject-2/3-level functionality. Production-grade env / config wiring
/// lands in a later task.
pub async fn build_dev_client() -> anyhow::Result<InProcessAppServerClient> {
    let config = Arc::new(
        ConfigBuilder::default()
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("failed to build config: {e}"))?,
    );

    let state_db = state_db::try_init(config.as_ref())
        .await
        .map_err(|e| anyhow::anyhow!("failed to init state db: {e}"))?;

    let client = InProcessAppServerClient::start(InProcessClientStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config,
        cli_overrides: Vec::new(),
        loader_overrides: LoaderOverrides::default(),
        strict_config: false,
        cloud_config_bundle: Default::default(),
        feedback: Default::default(),
        log_db: None,
        state_db: Some(state_db),
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        config_warnings: Vec::<ConfigWarningNotification>::new(),
        session_source: SessionSource::Cli,
        enable_codex_api_key_env: false,
        client_name: "lemurclaw".to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        experimental_api: true,
        mcp_server_openai_form_elicitation: false,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await
    .map_err(|e| anyhow::anyhow!("failed to start in-process app server: {e}"))?;

    Ok(client)
}

/// Parse a raw JSON body as a [`ClientRequest`], returning the captured
/// `id` (as raw JSON, preserving string/number shape) alongside the parsed
/// request. On parse failure the id is still best-effort extracted from the
/// input Value so a caller can log it.
pub fn parse_client_request(
    value: serde_json::Value,
) -> Result<(serde_json::Value, ClientRequest), serde_json::Error> {
    // Capture the raw `id` from the value BEFORE from_value consumes it.
    // Every ClientRequest variant carries an `id` field named `id` (typed as
    // `RequestId = string | number`), but pulling it back out of the enum
    // after deserialization is awkward; the untyped Value makes it a one-liner.
    let req_id_json = value.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let req = serde_json::from_value::<ClientRequest>(value)?;
    Ok((req_id_json, req))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_response_envelope_ok_emits_result_field() {
        let env = build_response_envelope(serde_json::json!(42), Ok(serde_json::json!({"x": 1})));
        let expected = serde_json::json!({
            "jsonrpc": "2.0", "id": 42, "result": {"x": 1}
        });
        assert_eq!(env, expected);
    }

    #[test]
    fn build_response_envelope_err_omits_data_when_none() {
        let env = build_response_envelope(
            serde_json::json!("abc"),
            Err(JSONRPCErrorError {
                code: -32601,
                message: "method not found".into(),
                data: None,
            }),
        );
        // Stringify + re-parse so we can assert key *absence* cleanly (a Map
        // lookup would also work, but this matches what hits the wire).
        let obj: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::to_value(&env).unwrap()).unwrap();
        assert_eq!(obj["jsonrpc"], serde_json::json!("2.0"));
        assert_eq!(obj["id"], serde_json::json!("abc"));
        let error = obj["error"].as_object().expect("error object");
        assert_eq!(error["code"], serde_json::json!(-32601));
        assert_eq!(error["message"], serde_json::json!("method not found"));
        assert!(
            !error.contains_key("data"),
            "data field must be absent when None, got: {error:?}"
        );
    }

    #[test]
    fn build_response_envelope_err_includes_data_when_some() {
        let env = build_response_envelope(
            serde_json::json!(7),
            Err(JSONRPCErrorError {
                code: -1,
                message: "boom".into(),
                data: Some(serde_json::json!({"k": "v"})),
            }),
        );
        let obj: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::to_value(&env).unwrap()).unwrap();
        let error = obj["error"].as_object().expect("error object");
        assert_eq!(error["data"], serde_json::json!({"k": "v"}));
    }

    #[test]
    fn serialize_server_event_lagged_envelope() {
        // The Lagged variant is the only one we can construct without a full
        // codex event payload; it's also the hand-rolled JSON shape worth
        // pinning down explicitly.
        let json = serialize_server_event(InProcessServerEvent::Lagged { skipped: 3 });
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, serde_json::json!({"type": "lagged", "skipped": 3}));
    }

    #[test]
    fn parse_client_request_extracts_id_and_method() {
        // turn/start is a real ClientRequest variant the frontend sends; its
        // envelope is `{method, id, params}`. We only assert the id round-trips
        // and the variant parses (full variant assertions are codex's job).
        let value = serde_json::json!({
            "method": "thread/list",
            "id": 123,
            "params": {"limit": 5}
        });
        let (id, req) = parse_client_request(value).expect("parse");
        assert_eq!(id, serde_json::json!(123));
        // Just assert we got *some* ClientRequest variant; the discriminant
        // string lives in codex_app_server_protocol and isn't worth coupling
        // to here.
        let _ = req;
    }

    #[test]
    fn parse_client_request_returns_err_for_garbage() {
        let value = serde_json::json!({"method": "no/such/method", "id": 1, "params": {}});
        assert!(parse_client_request(value).is_err());
    }
}
