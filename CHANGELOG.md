# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [unreleased]

- Add `Scale` for uniform scaling
- Add `Alignment`
- Attributes set by built-in Nodes take precedence over custom attributes
- Fix disconnected `MultiChoice` exit routes and insufficient bend clearance for short alternatives
- Add `Annotation` for checkpoints connected to any node, with optional ahead/behind indicators
- Add `ContinuationStart` and `ContinuationEnd` ellipsis markers for diagrams that continue beyond their displayed boundaries
- Add `Continuation` to mark omitted sections within a diagram

## [0.3.9] - 2026-08-15
- Fix `Optional` and `Repeat` padding so arrowheads fit within their advertised geometry
- Add Rust, Coal, Navy, and Ayu [`Stylesheet`] variants
- Bump `resvg` to 0.48

## [0.3.8] - 2026-07-16
- Fix `Stack` connectors overshooting the node's advertised width
- Use `fill-opacity` instead of `rgba()` for broader SVG renderer compatibility

## [0.3.7] - 2026-04-22
- Add the multi-column `MultiChoice` container

## [0.3.6] - 2026-04-12
- Add the direct streaming renderer API through `svg::Renderer` and `Node::render`
- Refactor node implementations into focused submodules
- Add benchmarks and expand API documentation

## [0.3.5] - 2026-04-11
- Improve rendering performance when the `visual-debug` feature is enabled

## [0.3.4] - 2026-04-11
- Add `NodeGeometry` and geometry-aware drawing to avoid repeated layout computation
- Escape user-supplied SVG text and attribute names and values
- Fix the missing `xmlns:railroad` declaration when `visual-debug` is enabled
- Bump `resvg` to 0.47

## [0.3.3] - 2025-06-06
- Switch to Rust edition 2024
- Bump `unicode-width` to 0.2
- Bump `resvg` to 0.45

## [0.3.2] - 2024-07-26
- Add light, dark, and render-safe [`Stylesheet`] variants
- Add optional `resvg` support and `render::to_png`

## [0.3.1] - 2024-07-14
- Add `svg::encode_attribute`

## [0.3.0] - 2024-07-12
- Replace the `htmlescape` dependency with internal escaping

## [0.2.0] - 2023-04-29
- Rename the `RailroadNode` trait to `Node`
- Add `FromIterator` implementations for container nodes
- Switch to stable Rust and edition 2021

## [0.1.1] - 2018-11-03
- Track the horizontal drawing direction so diagrams can render right-to-left arrows
- Make `Link` elements SVG 1.1 compliant and add link-target support

## [0.1.0] - 2018-10-24
- Initial release


[unreleased]: https://github.com/lukaslueg/railroad/compare/0.3.9...master
[0.3.9]: https://github.com/lukaslueg/railroad/compare/0.3.8...0.3.9
[0.3.8]: https://github.com/lukaslueg/railroad/compare/0.3.7...0.3.8
[0.3.7]: https://github.com/lukaslueg/railroad/compare/0.3.6...0.3.7
[0.3.6]: https://github.com/lukaslueg/railroad/compare/0.3.5...0.3.6
[0.3.5]: https://github.com/lukaslueg/railroad/compare/0.3.4...0.3.5
[0.3.4]: https://github.com/lukaslueg/railroad/compare/0.3.3...0.3.4
[0.3.3]: https://github.com/lukaslueg/railroad/compare/0.3.2...0.3.3
[0.3.2]: https://github.com/lukaslueg/railroad/compare/80674d2...0.3.2
[0.3.1]: https://github.com/lukaslueg/railroad/compare/0.3.0...80674d2
[0.3.0]: https://github.com/lukaslueg/railroad/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/lukaslueg/railroad/compare/0.1.1...0.2.0
[0.1.1]: https://github.com/lukaslueg/railroad/compare/0.1.0...0.1.1
[0.1.0]: https://github.com/lukaslueg/railroad/releases/tag/0.1.0

[`Stylesheet`]: https://docs.rs/railroad/latest/railroad/enum.Stylesheet.html
