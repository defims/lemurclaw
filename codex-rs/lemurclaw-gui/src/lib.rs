//! lemurclaw GUI: wry+tao webview + AppServerClient 驱动 + ipc shim.
//!
//! 子项目 2 填充:wry+tao 集成、AppServerClient(InProcess)、ipc_handler 收 JSON → ClientRequest、
//! next_event 循环 → serialize → tao proxy → evaluate_script、assets/(React 前端)。
//!
//! 当前进度(Task 2.3):
//! - webview 加载 `assets/dist/index.html`(经 file://,开发期)
//! - 注入 `window.__lemurclaw.onEvent` 桥(后端 → 前端 JSON 通道)
//! - `ipc_handler` 收 JS 的 ClientRequest JSON(前端 → 后端通道,目前只 println,Task 2.4 接 AppServerClient)
//! - tao `EventLoopProxy<GuiEvent>` 把后端事件投递回主线程主线程调 `evaluate_script`
//!
//! 待办(Task 2.4):构造 `InProcessAppServerClient`、`next_event` 循环、`request_handle` 转发。

use std::path::PathBuf;

use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;

/// 用户自定义 tao 事件:后端 worker task → 主线程的事件 JSON 字符串。
///
/// 后端(task 2.4 的 `next_event` 循环)把每个 `ServerNotification` / `ServerRequest`
/// 序列化为 JSON 字符串,经 `EventLoopProxy::send_event(GuiEvent::ServerEvent(json))`
/// 投递回主线程;主线程的事件 loop 在 `Event::UserEvent` 分支里把 JSON 经
/// `evaluate_script("window.__lemurclaw.onEvent(json)")` 推给前端。
#[derive(Clone, Debug)]
#[allow(dead_code)] // Task 2.3 only consumes ServerEvent in the event-loop match;
                   // construction lands in Task 2.4's next_event worker.
enum GuiEvent {
    ServerEvent(String),
}

/// Open the lemurclaw GUI window.
///
/// Loads the bundled React build (`assets/dist/index.html`) over `file://`,
/// installs the `window.__lemurclaw.onEvent` bridge so Rust can push events to
/// JS, registers an `ipc_handler` so JS can push `ClientRequest` JSON to Rust,
/// and runs the tao event loop.
///
/// This function owns the main thread: it enters the tao event loop and only
/// returns when the window is closed (or the loop exits). Callers should treat
/// it as a terminal entry point, like `codex_tui::run_main`.
///
/// `_proxy` is currently unused (kept on the stack as the channel for Task
/// 2.4's `next_event` loop); the `Event::UserEvent` arm that consumes it is
/// already wired so the wiring lands in one piece.
pub fn run_gui() -> anyhow::Result<()> {
    let event_loop: EventLoop<GuiEvent> = EventLoopBuilder::<GuiEvent>::with_user_event().build();
    let _proxy: EventLoopProxy<GuiEvent> = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("lemurclaw")
        .build(&event_loop)?;

    let dist_index = resolve_dist_index_html()?;
    let dist_url = format!("file://{}", dist_index.display());

    // Install the onEvent bridge up front so any Rust-driven evaluate_script
    // (including the Task 2.4 next_event loop) always has a handler. JS in
    // transport.ts may overwrite this with a JSON.parse-ing wrapper; that's
    // fine, both signatures are `(json: string) => void`.
    let init_script = "window.__lemurclaw = { onEvent: function(json) { console.log('[lemurclaw] onEvent (stub)', json); } };";

    let webview = wry::WebViewBuilder::new()
        .with_url(&dist_url)
        .with_initialization_script(init_script)
        .with_ipc_handler(move |request| {
            // JS → Rust. Task 2.4 forwards this to AppServerClient via
            // request_handle(). For now we just log so the channel can be
            // manually verified in the dev console.
            let body = request.body();
            println!("[lemurclaw] ipc_handler received: {body}");
        })
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                // Task 2.4 starts the AppServerClient worker here.
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

/// Resolve the absolute path to `assets/dist/index.html`.
///
/// `env!("CARGO_MANIFEST_DIR")` is the compile-time path Cargo bakes into the
/// binary for the crate that owns this source file (`lemurclaw-gui/`). It
/// therefore always points at the right place regardless of the current
/// working directory at runtime. We fall back to a few relative candidates
/// only if that baked path doesn't exist (e.g. crate relocated after build).
fn resolve_dist_index_html() -> anyhow::Result<PathBuf> {
    let baked = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("dist")
        .join("index.html");
    if baked.exists() {
        return Ok(baked);
    }

    let fallbacks: &[&str] = &[
        "assets/dist/index.html",
        "lemurclaw-gui/assets/dist/index.html",
        "codex-rs/lemurclaw-gui/assets/dist/index.html",
    ];
    for f in fallbacks {
        let p = PathBuf::from(f);
        if p.exists() {
            return Ok(p);
        }
    }

    anyhow::bail!(
        "lemurclaw-gui: assets/dist/index.html not found. \
         Run `npm run build` in codex-rs/lemurclaw-gui/assets/ first. \
         Searched baked={} plus fallbacks {:?}",
        baked.display(),
        fallbacks,
    );
}

/// Escape a string for safe embedding inside a JS double-quoted string
/// literal. Handles backslash, double-quote, newline, tab, and the two
/// U+2028/U+2029 line separators that are valid JSON but invalid in JS
/// string literals pre-ES2019 (and still risky in some engines).
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
