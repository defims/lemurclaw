//! 子项目 0 完成标准:验证 lemurclaw crate 的 Frontend/配置工作,以及 codex_tui 的关键符号可链接。
//! 不实际启动 TUI(那需要终端 + 会触发 CLI flag 冲突——见 Task 0.2 concern,待后续 task 修 argv 过滤)。

use lemurclaw::{Frontend, RuntimeConfig};

#[test]
fn frontend_enum_works() {
    let cfg = RuntimeConfig {
        agent_name: Some("test".to_string()),
        frontend: Frontend::Tui,
        ..Default::default()
    };
    assert_eq!(cfg.frontend, Frontend::Tui);
}

/// 验证 codex_tui::run_main 符号可从 lemurclaw 链接(证明 workspace 依赖正确)。
/// 取函数指针证明可见即可,不调用(调用需构造完整参数 + 终端)。
#[test]
fn codex_tui_run_main_is_linked() {
    let _ptr = codex_tui::run_main as *const ();
}
