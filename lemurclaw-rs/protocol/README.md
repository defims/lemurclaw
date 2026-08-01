# lemurclaw-protocol

This crate defines the "types" for the protocol used by lemurclaw CLI, which includes both "internal types" for communication between `lemurclaw-core` and `codex-tui`, as well as "external types" used with `lemurclaw app-server`.

This crate should have minimal dependencies.

Ideally, we should avoid "material business logic" in this crate, as we can always introduce `Ext`-style traits to add functionality to types in other crates.
