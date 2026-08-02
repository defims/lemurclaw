# lemurclaw-xtask

Release tooling for publishing the `codex-rs` workspace to crates.io under the
`lemurclaw-*` namespace.

This crate is **not** part of the `codex-rs` workspace (it has its own
`Cargo.toml` and builds standalone). The original `codex-rs/` source tree is
never modified by any subcommand; all output lands in a sibling `lemurclaw-rs/`
directory (only `target/` is `.gitignore`d; the source tree is tracked).

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

### `publish rename` — emit the parallel lemurclaw-rs workspace

Walks every publishable crate in `codex-rs/`, copies it into `lemurclaw-rs/`, and
rewrites:

- `Cargo.toml`: `codex-*` package names → `lemurclaw-*`, dependency keys
  renamed, `[[bin]]` sections dropped on `lib+bin` crates, `[patch.crates-io]`
  omitted from the workspace root.
- `.rs` sources: `use codex_foo::...` → `use lemurclaw_foo::...`, plus
  `extern crate` and bare path expressions. Rewrites are AST-driven (via
  `syn`); comment preservation falls back to a surgical line rewriter.

The subcommand finishes by running `cargo check --workspace` inside the new
`lemurclaw-rs/` directory so that any unrewritten `codex_*` reference surfaces
immediately.

```
cargo run -q -- publish rename
```

### `publish strip-v8` — exclude the V8 runtime from lemurclaw-rs/

Replaces the `code-mode` crate in `lemurclaw-rs/` with a V8-free stub (only
re-exports the protocol crate and provides session-provider stubs that fail at
runtime), removes the V8-only crates (`code-mode-host`, `v8-poc`) and the
`v8` / `deno_core_icudata` workspace dependency declarations, and regenerates
the lockfile. The resulting `lemurclaw` binary does not link `librusty_v8`
(~120 MB saved).

Run it after `publish rename`. It is idempotent and can be re-run safely. To
restore V8-backed code mode, run `publish restore-v8`.

```
cd xtask
cargo run -q -- publish strip-v8
```

### `publish restore-v8` — restore the V8 runtime in lemurclaw-rs/

Undoes `strip-v8`: re-copies the full V8 code-mode implementation from the
`codex-rs/` source tree (applying the codex-* → lemurclaw-* rename), recreates
the `code-mode-host` and `v8-poc` crate directories, and re-adds the
`v8` / `deno_core_icudata` workspace dependency declarations and V8-only
members. Use this when you need a V8-linking build from the same lemurclaw-rs tree.

Run it after `strip-v8`. It is idempotent.

```
cd xtask
cargo run -q -- publish restore-v8
```

## Exclusion policy

The following crates are excluded from the lemurclaw-rs workspace automatically:

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
