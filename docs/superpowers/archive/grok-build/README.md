# 已归档:grok-build 方案(abandoned)

放弃原因:grok-build 的沙箱(`xai-grok-sandbox` + `nix` crate)只支持 Unix
(Linux landlock / macOS seatbelt),**不支持 Windows**。lemurclaw 需要跨平台,
故转向 codex(支持 Windows)。

保留此归档作参考:
- `2026-07-17-lemurclaw-grok-build-gui-design.md` — 完整设计 spec(6 章)
- `2026-07-17-lemurclaw-skeleton-and-patch.md` — 子项目 0+1 实现计划(14 任务)

设计章节中**与上游无关的决策**(crate 形态、wry 进程内 IPC、React 前端、
TUI/GUI 可配置、等价 TUI 的 GUI)可能在新方案中延续,需重新 brainstorm 确认。
