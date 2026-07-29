# NEW SESSION PROMPT — paste this as the first message

在 `xtask/src/bundle.rs` 里加一个 post-merge 文案改写自动化:`rewrite_brand_display_text()`,
把合并后源码树里**用户面显示文案**的 `Codex`/`codex` -> `lemurclaw`。

完整规格在仓库根目录 `xtask-brand-rewrite-spec.md` —— **先完整读这个文件再动手**,里面有:
- 现有 `post_merge_fixups` 结构(`bundle.rs:2236` 分发,`:995` 调用,接收 `src_dir`=合并 crate 的 `src/`)
- 现有 `fix_*` helper 的 read/replace/write 模式(参考 `fix_otel_dangling_assignments` `:2566`)
- 完整的"旧串 -> 新串"替换表(CLI/TUI/Server 三组,逐文件逐字符串)
- **必须保留**的清单(env 变量、路径、flag 名、标识符、telemetry 名、模型 slug、OpenAI 基础设施 URL 等)
- 为什么必须用精确字符串 `str::replace` 而非 regex/通用 token 遍历(会误伤 `codex_home`/`codex_exec`/`codex://` 等)

## 关键点(不要偏离)

1. 新增 `fn rewrite_brand_display_text(src_dir: &Path) -> Result<()>`,放在 `fix_reqwest_tls_built_in_root_certs` 附近(`bundle.rs:2894` 左右)。用 `Vec<(&str,&str,&str)>` = `(相对 src_dir 的路径, old, new)` 的精确字符串对,逐文件 `fs::read_to_string` -> `raw.replace(old,new)` -> `fs::write`。

2. 在三个 cluster fixup 里调用:
   - `post_merge_fixups_cli`(`:2695`,现为空 `Ok(())`)→ 加调用
   - `post_merge_fixups_tui`(`:2635`)→ `Ok(())` 前加调用
   - `post_merge_fixups_server`(`:2630`,参数是 `_src_dir`)→ 去掉下划线变 `src_dir`,加调用

3. **范围**:只改用户直接读到的显示文字(CLI `--help`、TUI 屏幕、错误提示、doctor 输出、banner、tooltips.txt)。嵌在文案里的命令示例(`codex login`)也改成 `lemurclaw login`(因为 `bin_name` 已是 lemurclaw,示例要能复制粘贴跑通)。但**嵌在文案里的环境变量名(`CODEX_API_KEY`)/路径(`~/.codex`)/flag(`--codex-home`)保留不动**。

4. **绝不碰**:`CODEX_*` 环境变量、`~/.codex` 路径、`--codex-home` flag、`codex://` 协议、`codex_home`/`CodexAuth`/`CodexStatus` 等标识符、`codex.thread.*` telemetry、`gpt-5.x-codex` 模型 slug、`com.openai.codex`/`github.com/openai/codex`/`@openai/codex` 基础设施、系统提示词(`prompt.md`)、宠物名、`Codex.app` bundle 名。

5. **快照决策**:`"Ask Codex to do anything"` 占位符还出现在 52 个 `tui/src/**/snapshots/*.snap` 里。规格里标了 (a)/(b) 两个选项 —— 我要 (a):对该占位符也替换 `Ask Codex to do anything` -> `Ask lemurclaw to do anything` 于 `.snap` 文件(只这一个串,不碰 .snap 里其他 codex)。如果 xtask 遍历到 snapshots 目录,加这个特殊处理;否则在 fixup 里单独扫 `**/snapshots/*.snap`。

6. **tooltips.rs 的过滤器** `line.contains("codex app")` 必须和 tooltip 文本 `"Run 'codex app'"` 同步改成 `lemurclaw app`。

7. **远程服务器错误匹配串** 保留:`err.contains("codex plugins are disabled")`、`err.contains("update codex")` —— 它们匹配云服务返回,不是显示文案。只改配套的 remediation 消息。

## 验证

1. `cd xtask && cargo check` —— 必须编译通过(现有只有一个 dead-code warning)。
2. 跑 `cd xtask && cargo run -q -- publish bundle --target tui`,再 `cd ../publish && cargo build --bin lemurclaw` —— 必须编译通过。
3. `./target/debug/lemurclaw --help | head -3` → 显示 `lemurclaw CLI` + `Usage: lemurclaw`。
4. `./target/debug/lemurclaw exec --help | grep Usage` → `lemurclaw exec`。
5. `./target/debug/lemurclaw login --help` → `OPENAI_API_KEY | lemurclaw login` 且 `CODEX_ACCESS_TOKEN | lemurclaw login`(env 变量保留,命令改了)。
6. grep 审计:生产 .rs 里不应再有显示文案的 `Codex`/`codex`(排除 env/标识符/URL/slug 后)。
7. 抽查保留项:`--codex-home` flag 仍在 sandbox_setup.rs;`codex://` 仍在 history_ui.rs;`@openai/codex` 仍在 doctor。

## 备注
- `publish/` 是 gitignored 生成树,这个 fixup 让品牌重命名在每次 `xtask publish bundle` 后自动重生。
- 规格 `xtask-brand-rewrite-spec.md` 里的替换表是从一次手动改写 session 提炼的,已验证可编译且 `--help` 显示正确。照表实现即可。
- 每个 `old` 串要带够上下文,在文件内无歧义。同一文件内多次出现的相同串用 `raw.replace(old,new)`(替换全部)或 `replacen` 限次。
