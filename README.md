# oxygrace

A pure-Rust, headless renderer for [Grace](https://plasma-gate.weizmann.ac.il/Grace/)
(xmgrace) project files. It reads `.agr` / `.xvg` files and rasterizes them to
antialiased PNG images — no GUI, no X11, no external C libraries.

Grace is a venerable 2D plotting tool whose file formats are still produced by
a lot of scientific software (GROMACS and friends emit `.xvg`). oxygrace turns
those files into images anywhere — servers, CI pipelines, containers — without
installing Grace itself.

```bash
oxygrace plot.agr -o plot.png
```

## Gallery

All images below were rendered by oxygrace from the standard Grace example
projects.

| | |
|:---:|:---:|
| ![charts](docs/gallery/charts.png) | ![co2](docs/gallery/co2.png) |
| *Stacked / grouped charts, value labels, fills* | *Multi-panel layouts, annotations, callouts* |
| ![polar](docs/gallery/polar.png) | ![pie](docs/gallery/pie.png) |
| *Polar graphs* | *Pie charts with patterns and exploded slices* |
| ![tlog](docs/gallery/tlog.png) | ![terr](docs/gallery/terr.png) |
| *Log scales with power-format labels* | *X/Y error bars, clipped risers* |
| ![typeset](docs/gallery/typeset.png) | ![txttrans](docs/gallery/txttrans.png) |
| *Typesetting: sub/superscripts, Greek, fractions* | *Text transformations: mirror, slant, per-glyph rotation* |

## Features

- **Graph types** — XY graphs, charts (grouped and stacked), polar plots,
  pie charts, fixed-scale graphs, inset graphs and free multi-graph layouts.
- **Dataset types** — lines, scatter symbols (including character symbols),
  bars with error bars, X/Y error bars, hi–lo–open–close, boxplots,
  XY-radius circles, vector maps, per-point sizes and colors.
- **Axes** — linear, logarithmic, reciprocal and logit scales; automatic and
  explicit ticks; custom tick labels; tick-label formulas (`$t - 273.15`);
  decimal / general / exponential / scientific / engineering / computing /
  power formats; geographic (`110E`, `10S`) and date/time formats; staggered
  and skipped labels; zero axes, offset axes and per-side placement.
- **Annotations** — text strings, lines with arrowheads, boxes, ellipses and
  timestamps, in world or page coordinates.
- **Typesetting** — Grace's full text markup: fonts, colors, sub- and
  superscripts, under/overline, Symbol-font Greek, size changes, and text
  transformations (rotation, mirroring, slanting).
- **Styling** — Grace's default palette with `@map color` overrides, all 32
  fill patterns, the nine dash styles, legends, frames and background fills.
- **Fonts** — ships with the URW base35 fonts (metric-compatible with
  Grace's PostScript set) embedded in the binary; no font installation
  needed.
- **Compatibility** — reads files written by xmgr 4.x, Grace 5.x and
  QtGrace, including legacy formats; the tolerant parser skips unknown
  commands instead of failing.

The output aims to be *visibly identical* to Grace's own rendering — same
layout math, same defaults, same fonts — and is validated side by side
against the original Grace renderer over the full Grace example corpus.
(Exact pixel parity is a non-goal: oxygrace antialiases everything.)

## Installation

With a [Rust toolchain](https://rustup.rs) installed:

```bash
cargo install --git https://github.com/yesint/oxygrace
```

or clone and build:

```bash
git clone https://github.com/yesint/oxygrace
cd oxygrace
cargo build --release        # binary in target/release/oxygrace
```

## Usage

### Command line

```bash
oxygrace input.agr                  # writes input.png next to the input
oxygrace input.agr -o out.png       # explicit output path
oxygrace input.agr --width 1584 --height 1224   # override the page size
```

### As a library

```rust
let project = oxygrace::load("plot.agr")?;
let png = oxygrace::render_png(&project);
std::fs::write("plot.png", png)?;
```

## Not supported

- Smith charts (Grace itself never implemented drawing them).
- PostScript/SVG output — oxygrace renders PNG only.
- Grace's interactive command/scripting language — oxygrace renders project
  files, it does not evaluate scripts.

## License

GPL-2.0-or-later, matching Grace. The bundled
[URW base35 fonts](https://github.com/ArtifexSoftware/urw-base35-fonts) are
distributed under their own license.
