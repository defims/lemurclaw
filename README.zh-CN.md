# Lemurclaw

> Lemurclaw 是 [openai/codex](https://github.com/openai/codex) 的 fork,在 Codex
> agent 基础上新增了**原生 GUI 栈**和 **crates.io 发布工具链**。原有的 Codex
> TUI、agent 核心、云端集成保持不变。
>
> English: see [README.md](./README.md).

## 与上游的区别

- **GUI 栈** —— 除了原有的 TUI,还能以原生桌面窗口(`--frontend gui`)或浏览器
  应用(`--frontend webui`)运行 Codex。三种前端共用同一套 React/Vite/TypeScript
  界面,通过 JSON-RPC 与同进程的 AppServer 通信。
- **xtask 发布工具链** —— 把 `codex-*` 重命名为 `lemurclaw-*`,把 40+ 个 crate
  合并成 6 个,全树改写品牌,并可选地去掉 V8 运行时(二进制减小约 120 MB)。
- 发布工具**绝不修改原始 `codex-rs/` 源码树**,所有产物都落在 `lemurclaw-rs/`
  (仅 `target/` 被 gitignore)。

完整架构、构建、品牌改写参考见 [docs/lemurclaw.zh-CN.md](./docs/lemurclaw.zh-CN.md)。

## 快速开始 —— 构建并运行 GUI

lemurclaw 二进制从一个生成的 `lemurclaw-rs/` workspace 构建,它把上游的 `codex-*`
crate 重命名为 `lemurclaw-*` 并去掉 V8 运行时。这是推荐路径:产出的二进制小约
120 MB、不需要下载 `librusty_v8`,而且 GUI 对话路径本来就用不到 V8(code-mode
是可选的、默认关闭的能力)。

前置条件:Rust(stable)、Node.js(用于前端构建)。

```shell
# 1. 生成 lemurclaw-rs/ workspace 并去掉 V8(在仓库根目录执行)
cd xtask
cargo run -q -- publish rename     # codex-rs/ → lemurclaw-rs/,重命名 crate
cargo run -q -- publish strip-v8   # 把 code-mode 替换成无 V8 的 stub

# 2. 编译并运行 —— 在 lemurclaw-rs/ 下
cd ../lemurclaw-rs
cargo build -p lemurclaw

# 浏览器 UI(axum + WebSocket)—— 然后打开 http://127.0.0.1:8080
./target/debug/lemurclaw --frontend webui --port 8080

# 原生桌面窗口(wry + tao)
./target/debug/lemurclaw --frontend gui

# TUI(与上游 codex 相同)
./target/debug/lemurclaw
```

`lemurclaw-webui` 的 build 脚本会自动执行 `npm install && npm run build` 生成
内嵌的 React `dist/`。如果没有 Node,会回退到已提交的 `dist/` 并给出 warning。

> **为什么用 `lemurclaw-rs/` 而不是 `codex-rs/`?** GUI crate 作为 `codex-rs/`
> workspace 的成员存在是为了开发方便,但在那里编译 launcher 会拉入完整的 V8
> 运行时(约 120 MB 下载)。`lemurclaw-rs/` 是 lemurclaw 自包含的 workspace ——
> rename + strip-v8 给你一个不带 V8、能跑 TUI/GUI/WebUI 的二进制。

### 前端开发

开发共享 UI 时,在 `codex-rs/lemurclaw-webui/assets/`(lemurclaw-rs 构建嵌入的源)
下用 Vite 热更新:

```shell
cd codex-rs/lemurclaw-webui/assets
npm install
npm run dev    # Vite dev server,带热更新
npm run test   # vitest
```

## 发布到 crates.io

完整流水线(rename → 合并成 6 个 crate → 接入 fork → 发布)文档见
[docs/lemurclaw.zh-CN.md](./docs/lemurclaw.zh-CN.md#2-xtask-发布工具链) 和运行手册
[`.agents/skills/lemurclaw-upstream-sync/SKILL.md`](./.agents/skills/lemurclaw-upstream-sync/SKILL.md)。

## 三种前端如何选择

| 前端 | 形态 | 适用场景 |
|------|------|----------|
| `tui`(默认) | 终端 UI(ratatui) | 在终端里使用,与上游 codex 完全一致 |
| `gui` | 原生桌面窗口(wry + tao) | 想要独立桌面应用体验 |
| `webui` | 浏览器(axum + WebSocket) | 想在浏览器里用,或需要 DevTools 调试前端 |

三者共用同一套 React 界面和同一套 JSON-RPC 传输协议,后端都走同进程的
`InProcessAppServerClient`,不依赖外部 server。
