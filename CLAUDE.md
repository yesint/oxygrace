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

## Replicate Grace's formulas — do NOT guess

**The single most important rule for placement/geometry work.** For every
offset, gap, size, scale factor, or element position, find the exact formula in
the QtGrace6 / Grace source and replicate *that*, with a code comment citing the
file. Do not eyeball a constant until the montage "looks right" — a value tuned
to match one example is almost always wrong for another (this happened
repeatedly: a guessed axis-label offset matched co2 but was wrong everywhere
else; a guessed page mapping fixed co2 but broke bar).

Workflow when a visual element is misplaced:
1. Grep QtGrace6 for the relevant draw code (`drawticks.cpp`, `plotone.cpp`,
   `draw.cpp`, `graphutils.cpp`, `device.cpp`, `graphs.cpp`).
2. Read the actual expression (the magic constant, the justification, the
   per-version branch) and port it verbatim, citing the source in a comment.
3. Only then verify against the baseline. If a few px remain, confirm with the
   original `gracebat` before tuning — qtgrace and grace agree, and the
   reference is authoritative, not your intuition.

When the exact source value can't be reproduced (e.g. Grace measures a real
rendered bounding box we approximate), match the *intent* of the formula and say
so in a comment — don't substitute an unrelated guessed constant.

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

## Reference formulas (ported from QtGrace6 — cite the source in code)

These are the placement/geometry formulas we've reverse-engineered so far. Add
to this list as you port more; always keep the source reference.

- **View→device** is **isotropic**: `px = vx·side`, `py = page_h − vy·side`,
  `side = min(page_w, page_h)`; origin bottom-left in view space, Y flipped for
  the image (`rstdrv.cpp` `VPoint2gdPoint`, `page_scale = MIN2(w,h)`).
  `src/render/transform.rs`.
- **Default page**: our default is US-Letter landscape **792×612 px @ 72 DPI**.
  Files with `@page size W H` override it. (`DEFAULT_PAGE_WIDTH/HEIGHT` 733×538
  is Grace's *screen* default, not the hardcopy default.)
- **Old-format viewport rescale** (`graphs.cpp` `postprocess_project`): for
  `@version < 40005` force 792×612; for `version ≤ 40102` multiply every
  viewport (and view-loctype legend/object coords) by `get_page_viewport()` =
  `(width/side, height/side)` — pre-4.1.02 files store viewports as
  normalized-device-coords and must be stretched into the isotropic system.
  `src/parse/reader.rs` `postprocess_version`.
- **Line width px** = `linew · 0.0015 · side` (`MAGIC_LINEW_SCALE`, `globals.h`).
- **Font em px** = `charsize · 0.028 · side` (`MAGIC_FONT_SCALE`, `t1fonts.h`).
- **Tick mark length** = `0.02 · size` view units (`drawticks.cpp` `tsize`).
- **Tick label gap** `tl_offset = 0.01` view units (auto). Tick labels sit at
  `tl_offset` from the axis for inward ticks; `tsize + tl_offset` for outward
  (`drawticks.cpp` `vbase_tlabel`). x labels CENTER|TOP, y labels RIGHT|MIDDLE.
- **Axis label** anchor = `(distance to tick-label bbox edge) + tl_offset`
  (`drawticks.cpp` `vp_label_offset`). x label TOP-justified, y label
  MIDDLE-justified (rotated, centered). `src/draw/axes.rs`.
- **Symbol radius / bar half-width** = `0.01 · symsize` view units
  (`plotone.cpp` `drawxysym`, `drawsetbars`).
- **Chart bar grouping offset** accumulates `0.5·0.02·symsize` per set plus
  `bargap` (`plotone.cpp`); stacked charts accumulate y per category.
- **Font slot order** (no `@map font`) is Grace's t1lib order: 0 Times-Roman,
  **1 Bold, 2 Italic**, 3 BoldItalic, 4 Helvetica, 5 Helv-Bold, 6 Helv-Oblique,
  7 Helv-BoldOblique, 8 Courier… 12 Symbol, 13 ZapfDingbats. Verified against
  QtGrace's render of `tfonts.agr`. `src/font.rs`.
- **Default 16-color map** verbatim from `draw.cpp` `cmap_init`. `src/color.rs`.
- **Nine line-style dash patterns** and **32 fill patterns** copied from
  `patterns.h`. `src/render/canvas.rs`, `src/patterns.rs`.
- **Clipping**: fills/lines/bars/errbars clip to the graph viewport ±
  `VP_EPSILON 1e-4` (`draw.cpp` `clip_line`/`clip_polygon`); symbols/avalues
  are unclipped but skip points outside the world window (`is_validWPoint`).
  Out-of-domain scale values (log of <=0) map to **view 0**, never skipped
  (`xy_xconv_general`/`xy_yconv_general`).
- **Log ticks** (`drawticks.cpp` `calculate_tickgrid`): world bounds and
  `tick major` transform by log10 (major 10 = decades, 2 = octaves); minors
  at 2..nminor+1 multiples; `t_round` floors the start; >`MAX_TICKS` 256
  re-autoticks (`auto_ticks`/`nicenum`, graphutils.cpp).
- **Markup escapes** (`t1fonts.cpp` WriteString): \s/\S shift 0.4/0.6 of the
  *current* size then scale by 1/sqrt2 (cumulative); \n = baseline -1 em +
  carriage return; \v/\V/\h shifts; \+\- = 2^(1/4); \c..\C = +128
  charset; under/overline from font metrics.
- **Font order by version** (pars.yacc): `@version < 50001` = old ACE/gr
  order (bold-then-italic, Symbol at 8); newer = FontDataBase order
  (italic-then-bold, Courier 8..11, Symbol 12). `@map font` overrides.
  `src/font.rs` FONT_MAP_*.
- **Old-format fixups** (graphs.cpp `postprocess_project`): version <=40102
  also rescales view-loctype objects; 40200..=50005 ORs JUST_MIDDLE into
  string justs; <50001 selects the old font map.
- **Error bars** (`plotone.cpp` drawerrorbar): riser + cap of half-length
  `0.01*size`; riser clip cuts at `cliplen` with an open arrow `2*size`.
- **Arrowheads** (`plotone.cpp` draw_arrowhead): L = `0.01*length`,
  d = L*dL_ff, l = L*lL_ff; types 0 open / 1 filled / 2 bg-filled.
- **Axis label offset** = max(tick-mark extent, tick-label extent) +
  `tl_offset` (the TEMP bbox accumulates marks *and* labels).
- **Fixed graphs**: one shared world->view rate (min of x/y), viewport
  shrunk to the world aspect, centered (grace 5.1 definewindow).

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

**M2 (done):**
- `@page size W H` parsing (fixes aspect ratio across the corpus).
- Symbols (all 11 types, fill + outline).
- Set fills (polygon / baseline, flat color). *Hatch patterns + fill-between-sets deferred.*
- Bar charts (`@type bar`/`bardy`; grouped + stacked + plain; outline+fill).
- Legends (box, per-set line/symbol/box swatch, text).
- Line types (left/right stairs) + Grace's nine dash patterns + line-width scaling.
- Fix: old (pre-`@target`) files attach inline data to the current graph's set 0.
- *Deferred to later: SYM_CHAR glyph symbols, segment2/3 line types, hatch fill patterns.*

**M3 (done):**
- Log scale tick generation + labels (decades/octaves, minors 2..9), autotick
  fallback (nicenum), `tick place rounded`, `ticklabel skip`, power /
  scientific / engineering / computing label formats.
- Data clipping to the graph viewport (device clip mask); out-of-domain
  coords map to view 0 like `xy_yconv_general`.
- Annotation objects: string, line (with arrows), box, ellipse, timestamp.
- Specified ticks/labels (`tick spec`, `ticklabel IDX, "..."`, old
  `tick/ticklabel type spec` forms).
- Zero/alt axes with offsets and per-side placement (tick op /
  ticklabel op / label op); `ticklabel formula` ($t arithmetic);
  geographic (degreeslon/lat) and date/time formats (src/dates.rs,
  Julian-date conversions).

**M4 (partial):**
- Done: error bars (all xy d* types, riser clip arrows); full markup engine
  (\v \h \z \c upperset, under/overline, marks, Symbol Greek);
  version-aware font mapping (+ `@map font`); SYM_CHAR symbols; avalue
  point labels; stacked-chart refy for all elements; `Fixed` graph
  viewport; per-point xycolor/xysize.
- Open: hilo / boxplot / xyr-circle / vmap-arrow renderers; polar / pie /
  smith graph types; `.xvg` ergonomics; ASCII/CSV import.

See `/home/semen/.claude/plans/we-will-build-an-wobbly-elephant.md` for the
original plan.
