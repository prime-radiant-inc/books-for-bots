# Prebuilt binaries CI/release pipeline — design

**Date:** 2026-07-11
**Status:** Approved

## Goal

Agents that want `books-for-bots` should be able to fetch a prebuilt binary for
their platform from the GitHub Releases page instead of needing a Rust
toolchain. Every version tag produces a complete set of binaries for the
platforms agents actually run on.

## Triggers

- **Release builds:** push of a tag matching `v*` (e.g. `v0.1.0`).
- **Dry run:** `workflow_dispatch` on the release workflow builds all targets
  but skips release creation, so the matrix can be verified without tagging.
- **Continuous testing:** `ci.yml` runs on every push to `main` and on PRs.

## Target matrix

| Target triple | Runner | Notes |
|---|---|---|
| `x86_64-unknown-linux-musl` | `ubuntu-latest` | static; any distro, Alpine containers |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | glibc-linked |
| `aarch64-unknown-linux-musl` | `ubuntu-24.04-arm` | native ARM runner; Graviton/ARM containers |
| `x86_64-apple-darwin` | `macos-15-intel` | Intel Mac (`macos-13` was retired) |
| `aarch64-apple-darwin` | `macos-latest` | Apple Silicon |
| `x86_64-pc-windows-msvc` | `windows-latest` | Windows |

All builds are native (no cross-compilation, no `cross`/Docker). The musl rows
install `musl-tools` via apt and `rustup target add` the musl triple.

## Workflows

### `.github/workflows/ci.yml`

Single Linux job on push to `main` and on PRs:

1. checkout
2. stable Rust toolchain + `Swatinem/rust-cache`
3. `cargo fmt --check`
4. `cargo clippy --all-targets -- -D warnings` (included only if the current
   codebase passes; otherwise omitted rather than shipped failing)
5. `cargo test`

### `.github/workflows/release.yml`

**`build` job** — 6-row matrix, `fail-fast: false`:

1. checkout
2. stable Rust toolchain (+ target triple) + `rust-cache` keyed per target
3. musl rows: `sudo apt-get install -y musl-tools`
4. `cargo build --release --target <triple>`
5. `cargo test --target <triple>` where the target is natively runnable
   (all six rows are — every runner matches its target)
6. Package `books-for-bots-<tag>-<triple>.tar.gz` (`.zip` on Windows)
   containing the binary, `LICENSE`, and `README.md`
7. Upload as a workflow artifact

**`release` job** — `needs: build`, runs only on tag pushes (skipped for
`workflow_dispatch` dry runs):

1. download all artifacts
2. generate `SHA256SUMS` over all archives
3. `gh release create <tag>` with auto-generated notes, attaching all archives
   and `SHA256SUMS`

Permissions: `contents: write` on the release workflow only.

## Error handling

- `fail-fast: false` on the matrix so one platform's failure doesn't mask
  others.
- The release job requires all six builds to succeed: a release is always
  complete or absent, never partial.

## Artifact naming

`books-for-bots-v0.1.0-aarch64-apple-darwin.tar.gz` — tool name, tag, target
triple. Stable and predictable so provisioning scripts can construct URLs.

## Testing

- Dry-run the matrix via `workflow_dispatch` before the first tag.
- First real validation: tag `v0.1.0` and confirm the release contains six
  archives plus `SHA256SUMS`, and that a downloaded binary runs.

## Out of scope (YAGNI)

- Installers (shell/PowerShell), Homebrew taps, crates.io publishing
- Windows ARM, armv7, BSDs
- Code signing / notarization
