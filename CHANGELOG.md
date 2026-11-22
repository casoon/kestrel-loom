# Changelog

All notable changes to kestrel-loom are listed here. kestrel-loom follows
[Semantic Versioning](https://semver.org/): while the version is 0.x, a minor release may
change behavior.

## [Unreleased]

## [0.2.0] - 2026-11-22

### Added

- **Palette:** optional own colors (`bg`, `grid`, `text`, `up`, `down`, `line`, `crosshair`,
  `unchanged`) as hex strings. Pass `palette` to `createChart`, or call `view.palette(colors)`
  to switch at runtime without rebuilding the chart; `view.theme(isDark)` restores the built-in
  colors. Without a palette the built-in light and dark themes are unchanged. Core type
  `Palette`, WASM method `setPalette`.
