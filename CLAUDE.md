# CLAUDE.md

Guidance for working in the **oxygrace** repository.

## Project

Oxygrace is a pure-Rust, headless interpreter and renderer for **Grace**
(xmgrace) `.agr` project files and `.xvg` data files. It parses a file into an
in-memory model and rasterizes it to a PNG. No GUI, no editing.

The behavioural reference is Grace and the **QtGrace6** C/C++ port at
`/home/semen/install/QtGrace6`. We reimplement its *semantics*, not its code.
The `examples/*.agr` files (copied from QtGrace6) are the test corpus. We target
"visibly close" output, not pixel-perfect parity — fonts, UTF-8 and
antialiasing differ.

## Build / run / test

```bash
cargo build                       # build lib + bin
cargo run -- examples/axes.agr    # render to examples/axes.png
cargo run -- in.agr -o out.png --width 1466 --height 1076
cargo test                        # unit + corpus smoke tests
cargo clippy                      # lint (keep clean)
```

## Constraints (do not break these)

- **Pure Rust only**, no C library wrappers unless absolutely necessary.
- Command-language parsing uses the **`peg`** crate (`src/parse/grammar.rs`).
- Rendering stack: **`tiny-skia`** (rasterizer) + **`ttf-parser`** (glyph
  outlines) + **`png`** (output). All antialiased.
- **TrueType/OpenType fonts only** (no Type1). Strings are **UTF-8** (files are
  decoded leniently to tolerate legacy Latin-1).
- Idiomatic, simple, readable, documented code. No overengineering.

## Module layout

Use the **`<module>.rs` + `<module>/` directory** convention — **never
`mod.rs`**. (`src/model.rs` + `src/model/enums.rs`, etc.)

```
src/
  main.rs            CLI
  lib.rs             public API: load / load_str / render_png
  model.rs + model/  plot data model (Project→Graph→Set, Axis, Frame, enums, defaults)
  color.rs           default 16-color map + @map color overrides
  font.rs            embedded URW base35 OTFs, glyph outlines via ttf-parser
  text.rs            Grace string markup parser + glyph layout
  parse.rs + parse/  grammar.rs (peg), reader.rs (line loop + apply), data.rs (rows)
  render.rs + render/ canvas.rs (tiny-skia device primitives), transform.rs (coords)
  draw.rs + draw/    plot.rs (draw order), axes.rs (ticks/labels), sets.rs (data)
assets/fonts/        bundled URW base35 OTFs (embedded via include_bytes!)
examples/            *.agr test corpus (from QtGrace6)
tests/integration.rs grammar + transform unit tests + corpus smoke test
```

## Reference constants (from QtGrace6)

- Default page **733×538 px** at 72 DPI (`DEFAULT_PAGE_WIDTH/HEIGHT`).
- View→device is **isotropic**: `px = vx·side`, `py = page_h − vy·side`, where
  `side = min(page_w, page_h)` (origin bottom-left in view space, Y flipped for
  the image). See `src/render/transform.rs`.
- Line width px = `linew · 0.0015 · side` (`MAGIC_LINEW_SCALE`).
- Font em px = `charsize · 0.028 · side` (`MAGIC_FONT_SCALE`).
- Default 16-color map and 13 font slots (Times/Helvetica/Courier + Symbol +
  Dingbats → URW base35) are in `src/color.rs` and `src/font.rs`.
- Tick mark length = `0.02 · size` view units.

## Authoritative grammar source

The `examples/*.agr` files are the ground-truth grammar corpus — grow the peg
grammar to cover commands those files actually emit. The online Grace command
reference has drifted from what real files contain. The yacc grammar in
QtGrace6 `src/pars.yacc` is the formal reference.

The parser is **tolerant**: any unrecognized line becomes `Command::Unknown`
and is skipped, so one bad line never aborts a load.

## Milestone status

- **M1 (done)**: parse + render single/multiple XY graphs — frame, linear axes
  with major/minor ticks, tick labels, axis labels, title/subtitle, set
  connecting lines. Tolerant parsing of the full command vocabulary.
- **M2 (next)**: symbols, set fills + baseline, legend rendering, line types
  (stairs/segments), line-style dashes + fill patterns, bar charts.
- **M3**: error bars; bar/hilo/boxplot set types; full string markup; autotick;
  log/reciprocal/logit tick labelling.
- **M4**: annotation objects (line/box/ellipse/string); alt axes; special tick
  formats; polar/pie; `.xvg` ergonomics; ASCII/CSV import.

See `/home/semen/.claude/plans/we-will-build-an-wobbly-elephant.md` for the full
plan.
