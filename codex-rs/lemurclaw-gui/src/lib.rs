//! lemurclaw GUI: wry+tao webview + AppServerClient 驱动 + ipc shim.
//!
//! 子项目 2 填充:wry+tao 集成、AppServerClient(InProcess)、ipc_handler 收 JSON → ClientRequest、
//! next_event 循环 → serialize → tao proxy → evaluate_script、assets/(React 前端)。
//!
//! 当前进度(Task 2.1):仅打开一个空的 wry+tao 窗口,验证 wry/tao 依赖拉入 + tao EventLoop 跑通。
//! 后续 task 接 AppServerClient + IPC 双向 + React assets。

/// Open the lemurclaw GUI window.
///
/// Currently opens an empty wry webview displaying a placeholder HTML page,
/// driven by a tao `EventLoop`. Later tasks upgrade this to load the bundled
/// React assets and wire bidirectional IPC to the in-process AppServerClient.
///
/// This function owns the main thread: it enters the tao event loop and only
/// returns when the window is closed (or the loop exits). Callers should treat
/// it as a terminal entry point, like `codex_tui::run_main`.
pub fn run_gui() -> anyhow::Result<()> {
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoop};
    use tao::window::WindowBuilder;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("lemurclaw")
        .build(&event_loop)?;
    let _webview = wry::WebViewBuilder::new()
        .with_url("data:text/html,<html><body><h1>lemurclaw GUI</h1></body></html>")
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}
