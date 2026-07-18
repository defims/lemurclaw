//! lemurclaw GUI: wry+tao webview + AppServerClient 驱动 + ipc shim.
//!
//! 子项目 2 完成版:
//! - webview 加载 `assets/dist/index.html`(经 file://,开发期)
//! - 注入 `window.__lemurclaw.onEvent` 桥(后端 → 前端 JSON 通道)
//! - `ipc_handler` 收 JS 的 ClientRequest JSON,经 `request_handle()` 转发到 AppServerClient
//! - 后端 worker task 跑 `next_event` 循环,每个 `ServerNotification`/`ServerRequest` 序列化为
//!   JSON,经 tao `EventLoopProxy` 投递回主线程,主线程调 `evaluate_script` 推给 React
//!
//! 线程拓扑(关键):
//! - **主线程** 跑 tao EventLoop + 持有 wry WebView(macOS/Windows 强制 + WebView 非 Send)
//! - **tokio runtime** 独立线程(`std::thread` + `Runtime::new()`),承载 AppServerClient worker
//! - 主线程与 runtime 之间:
//!   - 主 → runtime:ipc_handler 用 `tokio::runtime::Handle`(Clone + Send)在 runtime 上 spawn
//!   - runtime → 主:worker task 用 tao `EventLoopProxy::send_event`(Send + Sync)投递事件
//!
//! 同源等效性:GUI 与 codex TUI 都消费同一份 `AppServerEvent` 流(经 `AppServerClient::next_event`),
//! 无 patch,无状态机复用 —— 是平行的另一个客户端。

mod assets;
mod backend;

use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;

/// 用户自定义 tao 事件:后端 worker task → 主线程的事件 JSON 字符串。
///
/// 后端的 `next_event` 循环把每个 `ServerNotification` / `ServerRequest` 序列化为 JSON,
/// 经 `EventLoopProxy::send_event(GuiEvent::ServerEvent(json))` 投递回主线程;主线程在
/// `Event::UserEvent` 分支里把 JSON 经 `evaluate_script("window.__lemurclaw.onEvent(json)")`
/// 推给前端。
#[derive(Clone, Debug)]
enum GuiEvent {
    ServerEvent(String),
}

/// Open the lemurclaw GUI window.
///
/// Spawns a tokio runtime on a background thread, builds the in-process
/// AppServerClient there, loads the bundled React build (`assets/dist/index.html`)
/// over `file://`, installs the `window.__lemurclaw.onEvent` bridge, registers
/// an `ipc_handler` that forwards `ClientRequest` JSON to the backend via
/// `request_handle()`, and runs the tao event loop on the main thread.
///
/// This function owns the main thread: it enters the tao event loop and only
/// returns when the window is closed (or the loop exits). Callers should treat
/// it as a terminal entry point, like `codex_tui::run_main`.
pub fn run_gui() -> anyhow::Result<()> {
    let event_loop: EventLoop<GuiEvent> = EventLoopBuilder::<GuiEvent>::with_user_event().build();
    let proxy: EventLoopProxy<GuiEvent> = event_loop.create_proxy();

    // Boot the backend (tokio runtime + AppServerClient + next_event worker)
    // on a background thread. If it fails to start we surface the error before
    // even opening the window so the user sees a real message instead of a
    // silent blank UI.
    let backend = backend::spawn(proxy)?;

    let window = WindowBuilder::new()
        .with_title("lemurclaw")
        .build(&event_loop)?;

    let entry_url = assets::entry_url();

    // Install the onEvent bridge up front so any Rust-driven evaluate_script
    // (including the next_event loop) always has a handler. JS in transport.ts
    // may overwrite this with a JSON.parse-ing wrapper; both signatures are
    // `(json: string) => void`.
    let init_script = "window.__lemurclaw = { onEvent: function(json) { console.log('[lemurclaw] onEvent (stub)', json); } };";

    let webview = wry::WebViewBuilder::new()
        .with_url(&entry_url)
        // Serve the embedded React build (see `assets.rs`). This is the
        // wry/Tauri standard pattern and avoids macOS WKWebView's rejection
        // of file://-loaded pages trying to fetch ES-module subresources
        // (origin `null` + crossorigin → CORS preflight failure).
        .with_custom_protocol(assets::PROTOCOL_SCHEME.to_string(), assets::handle)
        .with_initialization_script(init_script)
        .with_ipc_handler(move |request| {
            // JS → Rust. Forward the JSON body to the AppServerClient via
            // the request handle on the backend's tokio runtime.
            backend.handle_ipc(request.body());
        })
        .with_devtools(true)
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                // The next_event worker is already running on the backend
                // thread; nothing to kick off here.
            }
            Event::UserEvent(GuiEvent::ServerEvent(json)) => {
                // Escape for safe embedding in a JS string literal. JSON
                // itself is not JS-source-safe (</script>, U+2028, etc.) and
                // we're concatenating into a `"...{}"` template.
                let escaped = escape_js_string(&json);
                let script = format!("window.__lemurclaw.onEvent(\"{escaped}\")");
                if let Err(e) = webview.evaluate_script(&script) {
                    eprintln!("[lemurclaw] evaluate_script failed: {e}");
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}

/// Escape a string for safe embedding inside a JS double-quoted string
/// literal. Handles backslash, double-quote, newline, carriage return, tab,
/// and the two U+2028/U+2029 line separators that are valid JSON but invalid
/// in JS string literals pre-ES2019 (and still risky in some engines).
fn escape_js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            other => out.push(other),
        }
    }
    out
}
