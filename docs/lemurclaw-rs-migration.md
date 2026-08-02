# Migration: `publish/` → `lemurclaw-rs/` (常驻、git 跟踪、增量同步)

> **状态**:阶段 1-4 已全部完成(选项 B)。
>
> This document is in Chinese for the maintainer's review. English summary at
> the bottom.

## 1. 动机

当前 `publish/` 是**临时产物**:`xtask publish rename` 每次 clean-slate 整体重生成,
`publish/` 被 gitignore,可随时丢弃。这带来两个问题:

1. **lemurclaw 的「身份」不清晰**。lemurclaw 自有的 4 个 crate(`lemurclaw`、
   `lemurclaw-gui`、`lemurclaw-webui`、`lemurclaw-transport`)寄生在 `codex-rs/` 里,
   `codex-rs/` 本质是上游 OpenAI 的代码。lemurclaw 没有一个属于自己的、常驻的、可见的
   「家」。
2. **日常改动路径别扭**。改 lemurclaw 代码要改 `codex-rs/lemurclaw/`(寄生副本),再跑
   rename 同步到 `publish/` 才能验证。改的是副本,不是成品。

目标:把 `publish/` 改名成 `lemurclaw-rs/`,作为**常驻、git 跟踪的 lemurclaw workspace**。
和 `codex-rs/` 等价(都是 Rust workspace),但只装 lemurclaw 的东西:codex-* 改名产物 +
lemurclaw 自有 crate。日常直接在 `lemurclaw-rs/` 改 lemurclaw 代码;上游更新时 rename
增量同步 codex-* 部分。

## 2. 现状(改动前的基线)

```
codex-rs/                          ← 上游 workspace,git 跟踪
├── Cargo.toml                     ← workspace 根,~162 个成员
├── core/ tui/ app-server/ ...     ← ~120 个 codex-* crate(上游)
├── lemurclaw/                     ← lemurclaw 自有(寄生)
├── lemurclaw-gui/                 ← lemurclaw 自有(寄生)
├── lemurclaw-webui/               ← lemurclaw 自有(寄生)
└── lemurclaw-transport/           ← lemurclaw 自有(寄生)

publish/                           ← 临时产物,gitignored,每次 rename 重生成
├── Cargo.toml                     ← rename 生成
├── core/ tui/ ...                 ← codex-* 改名成 lemurclaw-*
├── lemurclaw/ ...                 ← lemurclaw 自有(从 codex-rs 复制)
└── target/                        ← 16GB 编译产物

xtask/src/rename.rs:36-52          ← clean slate: 备份旧 publish/ → 整体重生成
```

关键事实(已核实):
- codex-rs 约 162 个 crate 成员。
- 其中 8 个目录 leaf 带 `codex-` 前缀(`codex-api` 等),rename 时 leaf 改成 `lemurclaw-*`。
- rename **不处理孤儿目录**:上游删除的 crate,重生成后会残留(当前 clean-slate 模式无此
  问题,因为整体重建;增量模式需要新增处理)。
- `codex-rs/Cargo.lock` git 跟踪;`publish/Cargo.lock` gitignored。

## 3. 目标结构

```
codex-rs/                          ← 上游 workspace,保持纯净(可选:移出 lemurclaw 4 crate)
├── core/ tui/ app-server/ ...     ← codex-* crate(上游,只读性质)

lemurclaw-rs/                      ← 常驻 workspace,git 跟踪(不含 target/)
├── Cargo.toml                     ← workspace 根
├── Cargo.lock                     ← git 跟踪
├── core/ tui/ ...                 ← codex-* 改名产物(增量同步,上游更新时刷新)
├── lemurclaw/                     ← lemurclaw 自有(直接在这里改)
├── lemurclaw-gui/                 ← lemurclaw 自有
├── lemurclaw-webui/               ← lemurclaw 自有
└── lemurclaw-transport/           ← lemurclaw 自有
```

## 4. 增量同步逻辑(rename 的核心变化)

### 4.1 两类目录,不同处理

| 类别 | 源 | 目标 | rename 处理 |
|------|----|------|-------------|
| **codex-* 改名产物**(~120 个) | `codex-rs/{core,tui,...}` | `lemurclaw-rs/{core,tui,...}` | 逐 crate 覆盖(先删目标再复制 + 改名 + brand rewrite) |
| **lemurclaw 自有**(4 个) | `codex-rs/{lemurclaw,...}` 或已在 lemurclaw-rs | `lemurclaw-rs/{lemurclaw,...}` | **跳过**(已存在则不碰;仅首次初始化时复制) |

判断依据:crate 的 `package.name` 是否以 `lemurclaw` 开头(`rename.rs:254` 现有的
`is_excluded` 反转逻辑已能区分)。

### 4.2 首次初始化 vs 增量同步

rename 需要两种模式(新增分支):

- **初始化模式**(`lemurclaw-rs/` 不存在):完整生成,含 lemurclaw 4 crate。等价于当前行为。
- **同步模式**(`lemurclaw-rs/` 已存在):只增量更新 codex-* 改名产物,跳过 lemurclaw 自有。

判断:`lemurclaw-rs/Cargo.toml` 是否存在。

### 4.3 新增难点:孤儿目录处理

clean-slate 模式下,上游删除的 crate 自然消失(整体重建)。增量模式下,被删除的 crate
目录会残留在 `lemurclaw-rs/`。

处理方案:同步前对比「源 codex-rs 的 publishable 列表」与「目标 lemurclaw-rs 现有改名
产物目录」,删除目标中多余的(lemurclaw 自有目录不参与比对,绝不删)。

### 4.4 workspace manifest 和 lockfile

- `Cargo.toml`:每次同步都整体重写(它本来就是从源 manifest 计算出来的,增量没意义)。
- `Cargo.lock`:从 git 跟踪的版本出发,`cargo update` 增量更新(而非 strip-v8 现在的
  删除重生成,那会导致版本漂移——见 §6 风险 2)。

## 5. 改动清单

### 5.1 xtask(核心)

| 文件 | 改动 |
|------|------|
| `rename.rs:36-52` | 去掉 clean slate 的整体备份删除;改成「初始化 vs 同步」分支 |
| `rename.rs:discover_crates` | 返回结果分两组(codex 改名 / lemurclaw 自有) |
| `rename.rs:process_crate` | 同步模式下对 codex 改名组:先删目标目录再复制 |
| `rename.rs`(新增) | 孤儿目录清理函数 |
| `rename.rs`(常量) | `publish_root` → `lemurclaw-rs`,所有 `publish` 字样 |
| `strip_v8.rs:regenerate_lockfile` | 从「删除 Cargo.lock」改成「保留 + cargo update」 |
| `bundle.rs`、`forks.rs`、`main.rs`、`verify.rs` | 所有 `publish` 路径/输出文本改成 `lemurclaw-rs` |

### 5.2 仓库配置

| 文件 | 改动 |
|------|------|
| `.gitignore` | `/publish/` `/publish.forks/` → `/lemurclaw-rs/target/` `/lemurclaw-rs.forks/`(只忽略 target,跟踪源码) |
| 顶层(可选) | 决定 codex-rs/ 是否移出 lemurclaw 4 crate(见 §7) |

### 5.3 文档

- `README.md` / `README.zh-CN.md`:快速开始里 `cd ../publish` → `cd ../lemurclaw-rs`
- `docs/lemurclaw.md` / `docs/lemurclaw.zh-CN.md`:所有 publish 引用
- `xtask/README.md`、`.agents/skills/lemurclaw-upstream-sync/SKILL.md`:publish → lemurclaw-rs

## 6. 风险

1. **rename 的 7 个 bug 修复可能回归**。增量模式改变了 process_crate 的调用上下文(逐
   crate 删除而非整体重建),需重新验证:目录名改名(Bug1)、workspace deps path(Bug3-5)、
   default-run(Bug6)、proto 文件名(Bug7)。建议增量模式上线后,跑一次完整
   `cargo check --workspace` 对照。

2. **Cargo.lock 版本漂移**(已知问题)。strip-v8 现在删 lockfile 让 cargo 重生成,导致
   `rama-error` 等选了 0.3.0 稳定版(应为 0.3.0-alpha.4)。本迁移要把 lockfile 改成 git
   跟踪 + cargo update 增量,顺便修掉这个漂移。但首次要从 codex-rs/Cargo.lock 继承正确
   的版本基线。

3. **brand rewrite 的增量性**。`rewrite_brand_full` 是全树扫描,幂等(已改的不会再改)。
   增量模式下新增/更新的文件会被扫到,应 OK。但要验证:同步模式下如果只更新了部分
   crate,brand rewrite 是否需要全树跑(保守起见,全树跑,因为它依赖跨文件的一致性)。

4. **git 体积**。~120 crate 源码进 git 历史,每次上游同步是一个大 commit。可接受,但要
   心里有数(target/ 不进 git,16GB 不受影响)。

5. **codex-rs 的 lemurclaw 4 crate 去留**(见 §7)。如果移出,codex-rs 纯净但 4 crate
   失去 workspace.dependencies 解析;如果保留,codex-rs 不纯净但无额外工作。

## 7. codex-rs/ 的 lemurclaw 4 crate 去留(选项 B 已执行)

> **决定**:采用选项 B —— 4 crate 完全移出 codex-rs/,只在 lemurclaw-rs/ 维护。codex-rs
> 保持纯净,上游同步零冲突。代价:codex-rs 不再编译 lemurclaw 4 crate(它们只在
> lemurclaw-rs/ 编译)。初始化模式不再支持从零生成 lemurclaw-rs/(它是常驻 git 跟踪
> workspace,通过 git clone 获得初始状态)。
>
> 选项 C(顶层 workspace)因「上游同步冲突面变大」被否决(详见下方对比表)。

### 最初设想(选项 C 原文)

4 crate 搬到 lemurclaw-rs,codex-rs/Cargo.toml 用
`path = "../lemurclaw-rs/lemurclaw"` 引用。codex-rs 仍能编译它们,codex-rs 的 Cargo.toml
只多 4 行 path 依赖,上游 merge 冲突极小。

### 可行性修正:与「lemurclaw-rs 独立 workspace」冲突

验证发现一个根本约束(已核实):

1. **cargo 不支持嵌套 workspace**。一个目录只能属于一个 workspace。
2. lemurclaw 4 crate 用 `codex-config = { workspace = true }` 等依赖 codex-* —— 这要求
   它们和 codex-* 在**同一个 workspace**(因为 `workspace = true` 解析到所属 workspace
   的 `[workspace.dependencies]` 表)。
3. 这 4 crate 直接依赖 7 个 codex-*,而这 7 个又传递依赖 ~120 个 codex-*(光 codex-tui
   就 47 个)。若 lemurclaw-rs 是独立 workspace,它得把这 ~120 个 codex-* 全声明成 path
   依赖 —— 等于复制整个 codex-rs 的 workspace 表,不可行。

**结论**:选项 C 不能在「lemurclaw-rs 独立 workspace」前提下成立。要让它成立,只有一种
形态:**顶层 Cargo.toml 作为唯一 workspace,codex-rs/ 和 lemurclaw-rs/ 都不再是独立
workspace,而是顶层 workspace 的成员子目录。**

### 选项 C 的唯一可行落地形态

```
./Cargo.toml                        ← 顶层 workspace(唯一),含所有成员
./Cargo.lock
├── codex-rs/                        ← 子目录(不再是 workspace),含 codex-* crate
│   ├── core/ tui/ app-server/ ...
│   └── (lemurclaw 4 crate 已移走)
└── lemurclaw-rs/                    ← 子目录,含 lemurclaw 自有 crate
    ├── lemurclaw/                   ← codex-rs 通过 path 引用,workspace=true 解析到顶层
    ├── lemurclaw-gui/
    ├── lemurclaw-webui/
    └── lemurclaw-transport/
```

顶层 Cargo.toml 形态:
```toml
[workspace]
members = ["codex-rs/*", "lemurclaw-rs/*"]
# codex-* 的 workspace.dependencies 表从原 codex-rs/Cargo.toml 迁上来
[workspace.dependencies]
codex-core = { path = "codex-rs/core" }
codex-tui = { path = "codex-rs/tui" }
# ... ~120 条
lemurclaw = { path = "lemurclaw-rs/lemurclaw" }
lemurclaw-gui = { path = "lemurclaw-rs/lemurclaw-gui" }
# ...
```

### 这个形态的代价

| | 代价 |
|---|---|
| **上游同步冲突** | codex-rs/Cargo.toml 不再是 workspace 根(降级为普通目录),workspace 表迁到顶层。上游 merge 时,顶层 Cargo.toml 和 codex-rs/Cargo.toml 都可能冲突——**冲突面反而比现状大**(现状:冲突只在 codex-rs/Cargo.toml 的 4 行 members)。 |
| **构建命令变化** | `cargo build` 要在顶层跑(不在 codex-rs/ 或 lemurclaw-rs/)。justfile 的 `working-directory := "codex-rs"` 要改。 |
| **publish 流程** | xtask rename 读的源从 codex-rs/Cargo.toml 改成顶层 Cargo.toml;输出的 lemurclaw-rs/ 不再有独立 Cargo.toml(它是顶层成员)。 |
| **Bazel** | 如果 codex-rs 用 Bazel 构建(仓库有 BUILD.bazel),顶层 workspace 化要同步改 Bazel 配置。 |

### 重新评估:选项 C 是否仍最优

选项 C(顶层 workspace)的真实代价是**上游同步冲突面变大**——这与「codex-rs 保持纯净、
上游同步零冲突」的初衷部分相悖。对比:

| 方案 | codex-rs 纯净度 | 上游冲突面 | 4 crate 可独立编译 | 复杂度 |
|------|----------------|-----------|-------------------|--------|
| 现状(寄生) | 不纯净(含 4 crate) | 小(4 行 members) | 是 | 低 |
| 选项 A(保留副本) | 不纯净 | 小 | 是 | 低(两份副本) |
| 选项 B(完全移出) | **纯净** | **零** | 否(只在 lemurclaw-rs) | 中 |
| 选项 C(顶层 workspace) | 纯净 | **大**(顶层+codex-rs 两个 manifest) | 是 | **高** |

**建议重新考虑**:如果「上游同步零冲突」是首要目标,**选项 B**(完全移出,codex-rs 不再
编译 lemurclaw 4 crate)冲突面最小,只是失去「在 codex-rs 里直接编译 lemurclaw」的便利。
lemurclaw 4 crate 的开发完全可以移到 lemurclaw-rs/(顶层 workspace 形态下)或独立的
lemurclaw-rs workspace(接受 path 依赖手工维护)。

> **待 maintainer 拍板**:选项 C 的高冲突代价是否可接受?还是改选 B?这个决定影响 §8
> 阶段 4 是否执行、以及顶层 workspace 是否建立。

## 8. 分步执行计划(建议)

### 阶段 1:仅改名(低风险,可先做)
- `publish` → `lemurclaw-rs`(路径常量、.gitignore、文档)
- rename 仍 clean-slate 整体重生成(行为不变,只换目录名)
- lemurclaw-rs 仍 gitignored(只是名字变了)
- **验收**:rename + strip-v8 + cargo build 全流程跑通,产出在 lemurclaw-rs/

### 阶段 2:git 跟踪 + Cargo.lock 固化
- lemurclaw-rs/ 源码进 git(只忽略 target/)
- Cargo.lock git 跟踪,从 codex-rs/Cargo.lock 继承基线
- strip-v8 改成保留 lockfile + cargo update(修版本漂移)
- **验收**:strip-v8 后 rama 版本不漂移

### 阶段 3:rename 增量同步(核心,风险最高)✅
- 实现「初始化 vs 同步」分支
- codex-* 改名产物逐 crate 覆盖;lemurclaw 自有跳过
- 孤儿目录清理(递归发现所有 crate 目录,支持 ext/、utils/、memories/ 嵌套布局)
- 重新验证 7 个 bug 修复不回归 ✅
- **修复 strip_v8 的 rama 版本漂移 bug**:`cargo update` 会把 `rama-error`/
  `rama-macros`/`rama-utils` 从 `0.3.0-alpha.4` 升到 `0.3.0` 稳定版,与
  `rama-core 0.3.0-alpha.4` ABI 不兼容(OpaqueError 被移除)。新增
  `PRERELEASE_PINS` 常量,在 `cargo update` 后按依赖顺序(rama-utils →
  rama-macros → rama-error)逐个 `cargo update --precise` 回 alpha.4。
- **验收**:strip-v8 后 rama 全系停在 alpha.4;`cargo check --workspace` 通过

### 阶段 4:4 crate 迁移(选项 B)✅
- codex-rs/Cargo.toml 移除 4 个 members + 3 个 workspace.dependencies path 条目
- 删除 codex-rs/ 下 4 个 crate 目录(权威副本已在 lemurclaw-rs/)
- xtask rename.rs 新增 `OWN_CRATE_MEMBERS`/`OWN_CRATE_DEPS`/`OWN_CRATE_PACKAGE_NAMES` 常量
- `rewrite_workspace_manifest()` 新增 `own_members`/`own_deps` 参数,注入到生成的
  lemurclaw-rs/Cargo.toml(members 列表 + workspace.dependencies)
- `clean_orphan_dirs()` protected 集合改用硬编码常量(不再依赖 `discovered.own`)
- `rewrite_lockfile()` keep 集合注入自有 crate package names
- 初始化模式改为报错(lemurclaw-rs/ 必须通过 git clone 获得)
- **验收**:codex-rs `cargo check --workspace` 通过(5m29s);lemurclaw-rs
  `cargo check --workspace` 通过(35m53s,含 4 个自有 crate);sync 后自有 crate
  内容指纹不变;孤儿清理不误删自有 crate

## 9. English summary

Migrate the throwaway `publish/` into a persistent, git-tracked `lemurclaw-rs/`
workspace — lemurclaw's real home, on equal footing with `codex-rs/`. Day-to-day
lemurclaw edits happen directly in `lemurclaw-rs/`; upstream updates flow in via
incremental rename sync (only the codex-* → lemurclaw-* renamed crates are
refreshed; lemurclaw's own 4 crates are never overwritten). Four-phase rollout,
all complete: rename only → git-track + pin lockfile → incremental sync →
relocate the 4 crates out of codex-rs (Option B).

**Option B (§7, executed)**: the 4 own crates are fully removed from
`codex-rs/` — they live exclusively in `lemurclaw-rs/`. codex-rs stays pure
(zero upstream-merge conflict), but can no longer compile them. Initialization
from scratch is no longer supported (lemurclaw-rs/ is obtained via git clone).
Option C (top-level workspace) was rejected because it would increase the
upstream-merge conflict surface more than the status quo.
