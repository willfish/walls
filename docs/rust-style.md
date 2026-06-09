# Rust Style

This guide records the Rust conventions already expected in `walls`. It is not a
general Rust tutorial; use it to keep larger changes, especially TUI extraction
and validation work, consistent with the rest of the repo.

## Module Boundaries

- Keep modules aligned to one responsibility: config shape, validation,
  provider clients, provider orchestration, apply backends, state, or rendering.
- Prefer extracting cohesive helpers before a file becomes a mix of state,
  rendering, parsing, and I/O. The TUI split should move state, edit-session
  logic, command handling, provider-specific helpers, and rendering into
  separate modules.
- Keep public APIs narrow. Use `pub(crate)` for cross-module helpers inside a
  crate and `pub` only for APIs needed by another crate or integration tests.
- Avoid adding traits until there is a real second implementation or a test seam
  that is clearer than a plain function.

## Error Handling

- Use typed errors for public `walls-core` context APIs, following `WallsError`.
- Use `anyhow::Result` inside orchestration and provider internals where the
  caller mainly needs context and fallback behaviour.
- Add provider and operation context at async/network boundaries with
  `ProviderDescriptor::failure_scope` or an equivalent concrete path.
- Do not panic for user input, config, filesystem, network, or desktop-command
  failures. Reserve `expect` for invariant checks such as static fixtures or
  regex construction.

## Config And Validation

- Serde structs should mirror the user-facing config schema. Prefer explicit
  fields and `#[serde(default)]` over loosely typed maps when the shape is known.
- Disabled sources should stay non-blocking unless a story explicitly changes
  that contract.
- Full config validation and scoped TUI edit validation should share rules, but
  scoped validation must avoid unrelated global failures.
- Prefer structured validation diagnostics for new validation surfaces once
  #223 lands; until then, keep string messages actionable and include the config
  path being fixed.

## Filesystem And State

- State-changing operations should hold the existing state lock pattern rather
  than adding ad hoc locks.
- Writes that replace config, state, generated desktop entries, or downloaded
  provider cache files should use a temporary file followed by rename where
  practical.
- Keep path expansion and platform path discovery in `paths`/config helpers
  rather than duplicating environment handling.

## Async And Providers

- Provider clients should own request construction, timeout/retry policy, and
  response parsing. Orchestration should decide order, fallback, and apply
  behaviour.
- Network tests must stay hermetic by default. Use `wiremock`, local fixtures, or
  local TCP listeners; live-network tests should be ignored and explicitly
  labelled.
- Online provider failures should not prevent local/provider fallback unless the
  operation is explicitly user-requested and no fallback exists.

## Ownership And Data Shape

- Prefer borrowing slices and paths (`&[T]`, `&Path`) for read-only helpers.
- Return owned `PathBuf`/metadata where the result crosses async boundaries or is
  stored in state.
- Avoid cloning whole config/state values unless persistence or rollback needs a
  normalized owned copy.
- Use small typed enums for stable domain choices that are known at compile time;
  keep raw strings only when preserving user-facing extensibility.

## Tests And Benchmarks

- Add focused tests with the change. Broaden to integration tests when the change
  affects provider dispatch, state transitions, CLI/TUI behaviour, or apply
  semantics.
- Keep tests deterministic and offline-safe. Temporary directories should contain
  all files a test needs.
- TUI changes should preserve the existing layout contracts and smoke tests; add
  narrow tests around any extracted edit/render behaviour.
- Performance-sensitive work should update or add Criterion benchmarks rather
  than relying on intuition.

## Tooling Gates

- Keep the standard workspace gates green before publishing code changes:
  `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo test --workspace`.
- If a change touches optional features outside the default build, run the
  relevant explicit feature gate in addition to the standard workspace gates.
- New dependencies must pass `cargo deny`; duplicate-version exceptions need an
  exact-version reason in `deny.toml`.
