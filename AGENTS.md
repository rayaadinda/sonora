# Project Coding & Agent Rules — Sonora

## Mandatory Workflow Rules

1. **No Unnecessary Comments**: Do not write inline `//` comments, block `/* */` comments, section-divider banners, or boilerplate `///` / `//!` doc comments anywhere in the codebase. Code must communicate intent through expressive naming, strong types, and small focused functions. Only add a comment when strictly unavoidable (such as a `// SAFETY:` justification on an `unsafe` block or a non-obvious external invariant).
2. **No Tests**: Do not generate unit tests (`#[cfg(test)]`), integration tests (`tests/`), doc tests, benchmarks, or `[dev-dependencies]` test harnesses.
3. **Never Run `cargo` Commands**: The agent must never execute `cargo` (`cargo check`, `cargo build`, `cargo run`, `cargo clippy`, `cargo fmt`, `cargo test`, `cargo add`, etc.) in the terminal. Always let the user run `cargo` commands manually in their own terminal.

## Rust Engineering Standards

### Ownership, Borrowing & Performance
- Prefer borrowing (`&T`, `&str`, `&[T]`) in function parameters instead of taking owned values or calling `.clone()`.
- Derive `Copy` on small plain-data structs and tag-like enums (`<= 24` bytes, no heap fields). Never implement both `Copy` and `Iterator` on the same type.
- Use `Cow<'_, str>` when ownership is ambiguous and use `_else` combinators (`unwrap_or_else`, `ok_or_else`, `map_or_else`) to avoid eager allocations.
- Prefer lazy iterator chains over manual loops when transforming data, and avoid intermediate `.collect()` allocations when an iterator suffices.
- Box oversized enum variants or large stack allocations rather than passing large structures by value.

### Error Handling
- Use `thiserror` for structured domain errors (`AudioError`, `LibraryError`, `ConfigError`).
- Reserve `anyhow` strictly for the binary entry boundary (`src/main.rs`).
- Propagate errors with `?` and flatten control flow with `let ... else` early returns.
- Never use `unwrap()`, `expect()`, or `panic!` in production code paths.

### Structure, Naming & Imports
- Group imports in order: `std` → external crates → `crate::` / `super::`.
- Follow the Rule of Three before extracting a shared abstraction; prefer explicit enums over boolean flag parameters.
- Respect `[lints.rust]` and `[lints.clippy]` in `Cargo.toml`. Fix warnings at the root cause; only use `#[expect(clippy::...)]` when a lint is genuinely unavoidable.

## Ratatui & FLAC Audio Architecture Standards

- **Safe Terminal Lifecycle**: Always initialize and restore the terminal cleanly (`ratatui::try_init()` / `ratatui::try_restore()`) and install a panic hook that restores terminal state before printing panic output.
- **No Stdout/Stderr Pollution**: Never call `println!`, `eprintln!`, or `dbg!` while the alternate screen is active. Call `log_on_drop(false)` on Rodio's `MixerDeviceSink` and display runtime errors inside the TUI status toast.
- **Non-Blocking UI Loop**: Poll `crossterm::event::poll` at a smooth frame tick (~30 FPS) and never block the render loop on directory traversal, tag parsing, or image decoding. Offload library scanning to a background thread communicating via `std::sync::mpsc`.
- **Seekable Lossless FLAC Decoding**: Always construct Rodio decoders using `Decoder::builder().with_data(file).with_byte_len(len).with_seekable(true).with_hint("flac")` so random-access backward and forward seeking works reliably.
- **Graceful Resize Handling**: Every layout section must handle small terminal dimensions safely without clipping panics or integer underflow (use `saturating_sub`).
