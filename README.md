# books-for-bots

Convert an EPUB into a single YAML-headed Markdown file optimized for token-efficient reading by LLM agents.

## What it produces

```
output/<slug>/<slug>.md
output/<slug>/images/<basename>
```

The markdown file opens with a YAML frontmatter that lists every chapter with its absolute byte and line offsets in the file. The body is plain GFM Markdown. No HTML cleanup that requires judgment, no reflow, no surprises — just a deterministic structural translation of the book.

## Install

```sh
cargo install --path .
```

## Use

```sh
books-for-bots my-book.epub --output-dir output
```

Pass `--force` to overwrite an existing output directory.

## Why offsets in the frontmatter?

So an agent can read the markdown file with `--offset N --limit M` and seek directly to the chapter it wants, without parsing the whole file or walking a directory tree.

## Design

See [`docs/specs/2026-05-01-design.md`](docs/specs/2026-05-01-design.md) for the spec and [`docs/plans/2026-05-01-implementation.md`](docs/plans/2026-05-01-implementation.md) for the implementation plan.

## License

MIT.
