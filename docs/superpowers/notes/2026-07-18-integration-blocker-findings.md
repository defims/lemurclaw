# 集成阻塞实测发现(2026-07-18,执行子项目 0 Task 0.3 时)

执行 plan `2026-07-18-lemurclaw-skeleton-and-3rd-party-models.md` 时,实测推翻了
spec `2026-07-18-lemurclaw-codex-gui-design.md` §2.4 的核心集成假设。记录如下,供重新 brainstorm。

## 原 spec 假设(§2.4)

> codex-rs 作为 path dep 引用时,cargo 向上找到 vendor/code/codex-rs/Cargo.toml
> (它是独立 workspace),在那里解析 codex crate 的 workspace=true。

## 实测真相

### 发现 1:cargo workspace 默认扫描所有子目录
lemurclaw workspace 根(`/Users/def/lemurclaw/Cargo.toml`)默认把根目录下所有
含 Cargo.toml 的子目录当潜在成员扫描。`vendor/code/codex-rs/*` 的 ~127 个 crate
被扫到,当作 lemurclaw 成员,用 lemurclaw 根解析它们的 `workspace=true` → 失败
(lemurclaw 根没声明 121 个 codex-* 依赖)。

**解法:** `[workspace] exclude = ["vendor"]` 显式排除。验证可让 codex crate
不再被当 lemurclaw 成员。

### 发现 2:codex crate 的 workspace=true 边界仍复杂
即使 exclude 后,codex crate 仍通过传递依赖拉入复杂依赖图。

### 发现 3:alpha 依赖漂移(致命)
codex 的 `network-proxy` 依赖 `rama-core 0.3.0-alpha.4`,后者要求 `rama-error`
也是 alpha(`0.3.0-alpha.4`,含 `OpaqueError`)。但 lemurclaw 独立 workspace
**无 Cargo.lock**,cargo 重新解析时选了稳定版 `rama-error 0.3.0`(无 `OpaqueError`)
→ `rama-core` 编译失败。

codex-rs 自己的 `Cargo.lock` 锁定 `rama-error 0.3.0-alpha.4`,所以 codex 自己能编译
(实测 `cargo check -p codex-app-server-protocol` 在 codex-rs 目录下 7min 通过)。
lemurclaw 独立 workspace 继承不了这个 lockfile 的稳定性。

### 发现 4:cargo workspace 根约束(排除"进 codex workspace"方案)
cargo 要求 member crate 能从自身位置向上遍历找到 workspace 根。
- lemurclaw crate 在 `/Users/def/lemurclaw/crates/lemurclaw/`
- codex-rs 根在 `/Users/def/lemurclaw/vendor/code/codex-rs/`
- 从 lemurclaw crate 向上,先遇到 lemurclaw 根,不会"跨进" codex-rs workspace

实测:把 lemurclaw crate 加进 codex-rs/Cargo.toml 的 members(相对路径 `../../../crates/lemurclaw`),
cargo 报 `error inheriting edition from workspace root ... failed to find a workspace root`
—— member 物理上无法归属跨子树的 workspace。

**除非 lemurclaw crate 物理移进 vendor/code/codex-rs/ 内**(变成 submodule 内的代码)。

## 推翻的方案

- ❌ 独立 workspace + path dep(原 spec):exclude 能解成员扫描,但 alpha 漂移 + 无 lockfile 稳定性问题
- ❌ lemurclaw crate 进 codex-rs workspace:cargo 根约束,物理上行不通

## 待重评的可行方向

- A:lemurclaw crate 物理移进 vendor/code/codex-rs/(fork codex 加自己的 crate)
- B:独立 workspace + 复制 codex Cargo.lock + 强制 alpha 版
- C:直接 fork codex 仓库(lemurclaw 不是独立仓库)
- D:重新考虑上游(放弃 codex?)或重新考虑需求
