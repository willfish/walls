# Coverage Policy

CI enforces a workspace line coverage floor in `.github/workflows/ci.yml` with
`COVERAGE_LINE_FLOOR`.

The initial floor is `77%`. It was set from a local `cargo llvm-cov` baseline of
`78.07%` line coverage (`8691/11132` lines) on June 8, 2026. The floor is
intentionally a little below the measured baseline so harmless toolchain or
platform drift does not block unrelated changes.

To check coverage locally:

```bash
mkdir -p target/llvm-cov
nix develop -c cargo llvm-cov --workspace --summary-only --json --output-path target/llvm-cov/summary.json --fail-under-lines 77
nix develop -c jq -r '.data[0].totals.lines | "Line coverage: \(.percent | tonumber | . * 100 | round / 100)% (\(.covered)/\(.count) lines)"' target/llvm-cov/summary.json
```

When coverage rises and stays stable, ratchet `COVERAGE_LINE_FLOOR` up in the CI
workflow by one or two percentage points. If an intentional change must lower the
floor, call that out in the PR description with the before and after coverage
numbers, then raise it again in a follow-up once coverage is recovered.
