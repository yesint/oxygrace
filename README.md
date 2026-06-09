# oxygrace

A pure-Rust, headless interpreter and renderer for [Grace](https://plasma-gate.weizmann.ac.il/Grace/)
(xmgrace) `.agr` / `.xvg` plot files. It parses a Grace project and rasterizes
it to a PNG — no GUI, no external C libraries.

## Usage

```bash
cargo run -- examples/axes.agr            # writes examples/axes.png
cargo run -- plot.agr -o plot.png --width 1466 --height 1076
```

As a library:

```rust
let project = oxygrace::load("plot.agr")?;
let png = oxygrace::render_png(&project);
std::fs::write("plot.png", png)?;
```

## Status

Milestone 1: parses the Grace command language and renders XY graphs — frame,
linear axes with major/minor ticks, tick labels, axis labels, titles and set
connecting lines. Symbols, fills, legends, bars, error bars, log scales and
annotation objects are in progress (see `CLAUDE.md` for the roadmap).

## How it works

`tiny-skia` rasterizes antialiased paths; `ttf-parser` provides glyph outlines
from the bundled URW base35 fonts (the metric-compatible equivalents of Grace's
PostScript fonts); the command language is parsed with the `peg` crate.

The reference for Grace's behaviour is the QtGrace6 port; the `examples/` files
come from it and serve as the test corpus.

## License

GPL-2.0-or-later (matching Grace).
