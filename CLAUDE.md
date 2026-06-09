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

## Testing procedure (visual baseline comparison)

We validate against **QtGrace6** as the reference renderer. No pixel parity —
we check that the right *elements* are present and roughly placed.

```bash
scripts/baseline.sh [names...]   # qtgrace -> target/baseline/<name>.png
scripts/compare.sh  [names...]   # oxygrace -> target/out, montage -> target/compare
```

`scripts/compare.sh` builds oxygrace, renders each example, generates the
qtgrace baseline if missing, and writes a labelled side-by-side montage
(**ours left, reference right**) to `target/compare/<name>.png`. All outputs are
under the gitignored `target/`.

Review loop when working a milestone:
1. `scripts/compare.sh <name>` for the examples that exercise the feature.
2. Open `target/compare/<name>.png` and check the element checklist below.
3. Iterate until elements are present and placed correctly.

Element checklist: page size/aspect · frame · axes bars · major/minor ticks ·
tick labels (format/scale) · axis labels · title/subtitle · data lines · symbols
· fills · bars · error bars · legend (box + swatches) · annotation objects
(strings/lines/boxes/ellipses) · special/custom tick labels.

qtgrace runs headless via `QT_QPA_PLATFORM=offscreen`; **the `.agr` file must be
the first argument**, before `-hardcopy`, or it is ignored.

## Milestone status

**M1 (done):** parse the `@`-command language (tolerant), render single/multiple
XY graphs — frame, linear axes with major/minor ticks, tick labels, axis labels,
title/subtitle, set connecting lines.

Gaps found by baseline comparison, prioritized:

**M2 — plot content (makes the common plots right):**
- `@page size W H` parsing (we currently ignore it → wrong aspect everywhere). Quick, do first.
- Symbols (all 11 types, fill + outline).
- Set fills (polygon / baseline; flat color first, then the 32 fill patterns).
- Bar charts (`@type bar`/`bardy`; bar width from world spacing + `bargap`; outline+fill).
- Legends (box, one entry per set, line/symbol swatch, text).
- Line types (stairs, segments) + refined line-style dashes.

**M3 — scales + annotations (fixes log plots and the annotation-heavy files):**
- Log / reciprocal / logit scale tick generation + labels (powers of 10, minor 2..9).
- Autotick algorithm (nice round numbers when `tick major` absent / on autoscale).
- Data clipping to the frame rectangle.
- Annotation objects: string, line (with arrows), box, ellipse — used by 32/42 examples.
- Custom / special tick labels (`ticklabel type spec`, string labels like "Char read").
- Alt axes (offset bars) and zero axes (`type zero`).

**M4 — completeness:**
- Error bars (xydy/xydx/…); hilo / boxplot / xyz / xysize / xycolor set types.
- Full string-markup polish (Symbol-font Greek, all escapes).
- Polar / pie / smith graph types.
- `.xvg` ergonomics; ASCII/CSV import.

See `/home/semen/.claude/plans/we-will-build-an-wobbly-elephant.md` for the
original plan.
