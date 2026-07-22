# lemurclaw-xtask

Release tooling for publishing the `codex-rs` workspace to crates.io under the
`lemurclaw-*` namespace.

This crate is **not** part of the `codex-rs` workspace (it has its own
`Cargo.toml` and builds standalone). The original `codex-rs/` source tree is
never modified by any subcommand; all output lands in a sibling `publish/`
directory that is `.gitignore`d.

## Build

```
cd xtask
cargo check
```

## Subcommands

### `verify size` — measure compressed tarball size

Packages the 5 largest crates with `cargo package --no-verify --allow-dirty`
and reports the compressed `.crate` size against the crates.io 10MB limit.

```
cargo run -q -- verify size
```

### `verify patches` — probe fork-patch necessity

Temporarily comments out every entry under `[patch.crates-io]` in
`codex-rs/Cargo.toml`, runs `cargo check -p codex-tui -p codex-core`, then
restores the manifest regardless of outcome. Reports whether the ratatui /
crossterm / tungstenite forks are required for compilation.

```
cargo run -q -- verify patches
```

### `publish rename` — emit the parallel publish workspace

Walks every publishable crate in `codex-rs/`, copies it into `publish/`, and
rewrites:

- `Cargo.toml`: `codex-*` package names → `lemurclaw-*`, dependency keys
  renamed, `[[bin]]` sections dropped on `lib+bin` crates, `[patch.crates-io]`
  omitted from the workspace root.
- `.rs` sources: `use codex_foo::...` → `use lemurclaw_foo::...`, plus
  `extern crate` and bare path expressions. Rewrites are AST-driven (via
  `syn`); comment preservation falls back to a surgical line rewriter.

The subcommand finishes by running `cargo check --workspace` inside the new
`publish/` directory so that any unrewritten `codex_*` reference surfaces
immediately.

```
cargo run -q -- publish rename
```

## Exclusion policy

The following crates are excluded from the publish workspace automatically:

- `codex-bwrap`, `codex-thread-manager-sample` — bin-only samples
- crates with `publish = false` in their manifest
- crates under any `tests/` directory (test-support helpers)
- `codex-test-binary-support`, `codex-collaboration-mode-templates`
- existing `lemurclaw-*` crates (they have their own publish path)

For `lib + bin` crates, the `[[bin]]` sections are dropped along with
`src/bin/` and `src/main.rs`; the library target is what gets published.

## Out of scope

- Phase 2 utils merging (23 `codex-utils-*` → 4 `lemurclaw-utils-*` bundles)
- Publishing the ratatui / crossterm / tungstenite forks as separate crates
  (decided by `verify patches`)
- CI automation for the publish pipeline
