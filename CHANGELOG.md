# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-07-11

### Added
- `--version` flag so agents and provisioning scripts can check which build they have.

## [0.1.0] - 2026-07-11

### Added
- Initial release: convert an EPUB into a single YAML-headed Markdown file with
  chapter-level byte and line offsets, plus extracted images.
- Prebuilt binaries for Linux (x86_64 gnu/musl, aarch64 musl), macOS
  (arm64, x86_64), and Windows (x86_64), published on tagged releases with
  SHA256 checksums.

### Fixed
- EPUB-internal paths are normalized to forward slashes at the load boundary,
  fixing chapter-title resolution on Windows.
