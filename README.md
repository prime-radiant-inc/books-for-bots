# books-for-bots

Convert an EPUB into a single YAML-headed Markdown file optimized for token-efficient reading by LLM agents.

## Why

EPUB is a great format for humans and a terrible format for agents.

A typical agent reading a book has to (a) unpack the zip, (b) parse the OPF manifest, (c) follow the spine, (d) parse each XHTML document, (e) chase cross-document references, (f) do all of that without exhausting its context window. Most don't even try; they paste a chapter at a time from clipboard.

`books-for-bots` flips that. Each book becomes one plain GFM Markdown file with a YAML frontmatter listing every chapter and its **absolute byte and line offsets**. An agent can:

```
Read book.md --offset 412 --limit 200
```

…and land directly on Chapter 1's heading, no parsing, no traversal. The frontmatter is the chapter-level navigation API.

## What you get

```
output/<slug>/<slug>.md
output/<slug>/images/<basename>
```

The Markdown file opens with frontmatter that looks like this (see [`examples/alice/`](examples/alice/) for the full output):

```yaml
---
title: "Alice's Adventures in Wonderland"
authors: [Lewis Carroll]
published: 2008-06-27
language: en
source_file: examples/alice/alice-pg11-images.epub
chapters:
  - title: CHAPTER I. Down the Rabbit-Hole
    line:         94
    byte:       2866
  - title: CHAPTER II. The Pool of Tears
    line:        158
    byte:      14524
  - title: CHAPTER III. A Caucus-Race and a Long Tale
    line:        216
    byte:      25801
  - title: CHAPTER IV. The Rabbit Sends in a Little Bill
    line:        312
    byte:      35010
  ...
---

## CHAPTER I. Down the Rabbit-Hole

Alice was beginning to get very tired of sitting by her sister...
```

Below the frontmatter is plain GFM: `## Chapter Heading`, `**bold**`, `*italic*`, `[link](href)`, `[^footnote]`, GFM pipe tables, fenced code blocks. No embedded HTML except where GFM requires it (table cell `<br>`).

Numeric offsets are leading-padded to a fixed 10-character field so the frontmatter byte size is invariant. YAML plain-scalar parsing strips that padding, so consumers parse them as integers. Padding is leading (not trailing) because trailing whitespace gets eaten by editors and pre-commit hooks.

## Use

```sh
cargo install --path .
books-for-bots my-book.epub --output-dir output
```

Pass `--force` to overwrite an existing output directory.

That's the entire CLI surface. No flags for "include or skip footnotes," no flags for "merge or split chapters." The tool does one specific thing.

## Example

[`examples/alice/`](examples/alice/) contains Project Gutenberg's _Alice's Adventures in Wonderland_ (PG #11, public domain) and the converted output. 1832 lines of Markdown, 12 chapters with clean offsets, every chapter's heading reachable via the frontmatter byte position.

To regenerate it:

```sh
books-for-bots examples/alice/alice-pg11-images.epub --output-dir examples/alice/output --force
```

Then verify the offsets work:

```sh
# Land on Chapter VII's heading using the byte offset from the frontmatter:
dd if=examples/alice/output/alice-s-adventures-in-wonderland-lewis-carroll/alice-s-adventures-in-wonderland-lewis-carroll.md \
   bs=1 skip=76563 count=80 2>/dev/null
# → ## CHAPTER VII. A Mad Tea-Party
#
#   There was a table set out under a tree...
```

## Design principles

1. **Deterministic.** Same input → byte-identical output. No timestamps, no random ordering, no pretty-printing variability. Two runs always agree.
2. **No agentic judgment.** Every transform is a fixed rule. The tool doesn't decide which images are "decorative," doesn't guess whether a paragraph is "important," doesn't summarize. If it's in the spine, it's in the output.
3. **Faithful to source.** All text is preserved. Whitespace is collapsed where browsers would collapse it, preserved where they would preserve it (`<pre>`). Smart quotes, em-dashes, accented characters — all intact.
4. **One file per book.** Books are immutable. Treat the converted Markdown as immutable too. Offsets are stable seek targets.

## How it's built

A five-stage Rust pipeline:

1. **`load`** wraps the [`epub`](https://crates.io/crates/epub) crate. Returns a typed `Book` with metadata, ordered spine documents, and image bytes keyed by manifest path.
2. **`extract`** uses [`scraper`](https://crates.io/crates/scraper) (which is built on `html5ever`) to parse each spine document into a real DOM, then walks the DOM into a `Block`/`Inline` IR. Recognizes paragraphs, headings, lists, tables, blockquotes, fenced code, images, footnote references and definitions, anchors. Treats `<div>`/`<span>`/`<section>` as transparent. Drops empty elements.
3. **`assemble`** stitches spine documents into a chapter sequence. Resolves chapter titles by priority (TOC label → first H1/H2 → HTML `<title>` → filename). Namespaces footnote IDs per chapter (`[^cN-id]`). Rewrites cross-chapter links to GFM auto-slugs of their target chapter. Drops "running header" spine docs (Calibre/print-layout artifacts that just contain `<h1>Book Title</h1>`).
4. **`render`** serializes the `Block` tree to GFM Markdown into a `String`, recording the body-relative byte and line position of each chapter heading. Hand-written serializer; no `pulldown-cmark` round-trip. Handles GFM pipe escaping, backtick-fence widening, and heading-text whitespace collapse.
5. **`write`** computes the YAML frontmatter (with leading-padded offsets so its size is invariant), concatenates frontmatter + body, writes the markdown file, and copies referenced images to `images/`.

The whole binary is around 2,000 lines of Rust. Statically linked, no runtime dependencies.

## Real-world quirks

The tool runs cleanly across a wide range of EPUBs. A few patterns are worth knowing:

- **XHTML self-closing `<script />`** in `<head>` would otherwise break HTML5 parsing. The `extract` module strips `<head>` before parsing.
- **Calibre-split spine docs** (one spine document per print page) leave behind "running header" pages whose only content is `<h1>Book Title</h1>`. Those get dropped.
- **Embedded TOC chapters** in the source EPUB get faithfully transmitted but their internal links collapse to chapter-level slugs (the YAML frontmatter is the navigation API; the embedded TOC is decorative).
- **Footnote markup variations**: `<sup><a>` wrappers, `epub:type="noteref"`, plain `<a href="other.html#fnX">N</a>` with short marker text — all detected and emitted as `[^id]` references. Calibre's `#filepos…` fragment style is supported.
- **Source artifacts** (pirate-site watermarks, leftover XML escapes, page-break HTML comments) are transmitted as-is. Garbage in, garbage out — the tool is honest about what's in the book.

## Build and test

```sh
cargo build --release
cargo test
```

Tests are entirely synthetic. The fixture builder under `tests/common/` constructs in-memory EPUBs at test time using `epub-builder`. No `.epub` files of any kind are committed (except the public-domain Alice in `examples/`).

## Documentation

- [`docs/specs/2026-05-01-design.md`](docs/specs/2026-05-01-design.md) — design spec
- [`docs/plans/2026-05-01-implementation.md`](docs/plans/2026-05-01-implementation.md) — TDD implementation plan

## Credits

The example book is _Alice's Adventures in Wonderland_ by Lewis Carroll, [Project Gutenberg eBook #11](https://www.gutenberg.org/ebooks/11). Public domain. Project Gutenberg's terms of use are included with the source EPUB.

## License

MIT.
