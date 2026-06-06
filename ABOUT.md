# books-for-bots

> Rust CLI that converts EPUBs into a single YAML-headed Markdown file with per-chapter byte and line offsets, giving LLM agents a navigation API for token-efficient reading.

**Family:** agent-libs · **Type:** tool · **Lifecycle:** production · **Owner:** obra

## What it does
books-for-bots is a Rust CLI that converts an EPUB into a single GFM Markdown file with YAML frontmatter listing every chapter and its absolute byte and line offsets. This lets an LLM agent jump directly to a chapter (e.g. `Read book.md --offset 412 --limit 200`) without unpacking the zip, parsing the OPF manifest, or traversing the spine. Offsets are fixed-width leading-padded so the frontmatter byte size stays invariant. Output is written to `output/<slug>/<slug>.md` plus an `images/` directory.

## How it fits
- Depends on: — (no internal prime-radiant-inc deps in Cargo.toml)
- Used by: — (output consumed by LLM agents reading books)
- External: — (local EPUB input, no network services)

## Runtime & data
- Runs: CLI (Rust binary)
- Data in: EPUB files
- Data out: single YAML-headed GFM Markdown file plus extracted images

<!-- Maintained by the maintaining-project-map skill. Do not hand-edit; regenerated. -->
