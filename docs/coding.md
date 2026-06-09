# Coding Conventions

Read this file before writing or modifying any Rust code in this repository.

## Ownership and Borrowing

- Accept `&str` instead of `&String`, `&[T]` instead of `&Vec<T>` — more general and avoids forcing callers into specific container types.
- Accept owned types (`String`, `Vec<T>`) only when the function needs to store or move the data.
- For flexible ownership transfer, accept `impl AsRef<str>` when borrowing suffices. Use `impl Into<String>` sparingly — only when the function genuinely needs ownership and callers hold diverse types. Prefer accepting `String` directly if most callers already have one; the added genericity rarely justifies the API complexity.
- Prefer restructuring code over cloning to satisfy the borrow checker.
- Watch for cloning in error construction — `.clone()` into error enum fields inside `.map_err()` is a common LLM pattern. Consider taking ownership where possible.

## Type System

- Use enums and newtypes where raw primitives or strings encode domain semantics.
- Use builders for complex construction with many optional parameters. Validate at build time, not after construction.
- Prefer generics (`impl Trait` / `<T: Trait>`) by default. Use `dyn Trait` only when heterogeneous collections or dynamic dispatch are genuinely needed.

## Error Handling

- Propagate errors with `Result` and the `?` operator. `unwrap`/`expect` are denied by Clippy in all code including tests.
- **Library code:** define concrete error enums with `thiserror`.
- **Application code:** use `anyhow::Result` with `.context()` for actionable messages.
- Error messages should be lowercase, no trailing punctuation, and concise.
- Do not swallow errors silently; log or return at the boundary.

## API Design

- Keep public API surface small and documented with `///` where behavior is not obvious.
- Omit `get_` prefixes for getters: `fn field(&self) -> &T`.
- Implement `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Default`, and `Display` for public types where meaningful. Derive where possible.
- Return values instead of mutating out-parameters.

## Memory and Performance

- Pre-allocate collections when size is known: `Vec::with_capacity(n)`, `String::with_capacity(n)`.
- Avoid unnecessary `format!()`; it always allocates.
- Avoid repeated `.to_string()` / `.to_owned()` inside loops.
- Profile before optimizing; measure before and after.

## Module Organization

- Default to private. Use `pub(crate)` for crate-internal helpers. Expose `pub` only for intentional API surface.
- One concept per module — usually one primary type with its impls.
- Place public items before private items in files.
- Enum variants and trait methods are always public if the enum/trait is public.

## Extension Contracts

- Keep source manifests free of legacy `entrypoint` fields. Installable packages expose executables through `packages/<id>/bin/<id>`.
- Runtime registration top-level capabilities declare only capabilities the extension provides/owns. Action and route capabilities are requirements/grants and must not be duplicated as provided capabilities unless this extension actually owns them.
- `workspace-indicator` should not talk to providers directly; use spindle action invocation and continuation handles.
- Provider extensions should keep external protocol code isolated and expose a small spindle surface.

## Unsafe Policy

- Workspace `Cargo.toml` sets `unsafe_code = "forbid"`. Crates inherit that by default and add `#![forbid(unsafe_code)]` in `lib.rs` and `main.rs`; SketchyBar uses a crate-local `unsafe_code = "deny"` exception for its FFI bridge.
- `unsafe_code` is forbidden except for the SketchyBar Mach IPC FFI bridge.
- Keep unsafe localized to the bridge module, add a nearby safety comment, and expose only a safe public API.
- Do not use unsafe to fight the borrow checker.

## Documentation

- Add `//!` crate-level docs in `lib.rs`: brief description and usage notes.
- Document every public item with `///` where behavior is not obvious.
- Use `?` instead of `unwrap()` in doc examples.

## Recommended Libraries

Add these dependencies when the use case arises. Do not reimplement what they provide.

| Use case | Crate | When to add |
|---|---|---|
| Library error types | `thiserror` | Any crate that defines its own error enums |
| Application error handling | `anyhow` | Binary crates or top-level error propagation |
| Serialization | `serde` + format crate (`serde_json`, `toml`, etc.) | Any struct that crosses a serialization boundary |
| CLI argument parsing | `clap` (derive) | Binary crates with CLI arguments |

## Style

- Run `task fmt` before committing. Formatting uses **nightly** `rustfmt` with unstable options enabled in [rustfmt.toml](../rustfmt.toml).
- Clippy: `task lint` runs `cargo clippy ... -- -D warnings`. See [docs/tooling.md](../docs/tooling.md) for the full Clippy policy.
- When adding a crate root, copy the `#![warn(clippy::pedantic, clippy::nursery, clippy::cargo)]` attributes from existing crate roots.
- Prefer iterators over explicit loops when the transform chain is readable.
- Use `usize` for indices, not `i32` or `u32`.
