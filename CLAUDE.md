# CLAUDE.md

Guidance for working in the **oxygrace** repository.

## Project

Oxygrace is a pure-Rust interpreter and renderer for **Grace** (xmgrace)
`.agr` project files and `.xvg` data files. The core library (`oxygrace`)
parses a file into an in-memory model, rasterizes it (PNG/pixmap) or emits
SVG, and serializes the model back to `.agr` (`save_str`/`save`).
`oxygrace-cli` is the headless renderer; `oxygrace-gui` (egui/eframe,
native + wasm) is the interactive editor — see `docs/gui-analysis.md` for
the toolkit decision and architecture. All GUI milestones (G1–G5) are
complete; details in the Milestone status section below.

The behavioural reference is Grace and the **QtGrace6** C/C++ port at
`/home/semen/install/QtGrace6`. We reimplement its *semantics*, not its code.
The `examples/*.agr` files (copied from QtGrace6) are the test corpus. We target
"visibly close" output, not pixel-perfect parity — fonts, UTF-8 and
antialiasing differ.

## Build / run / test

The repo is a **virtual cargo workspace** with three member crates:
`oxygrace/` (core library), `oxygrace-cli/` (headless renderer; the
installed binary is named `oxygrace`), `oxygrace-gui/` (egui editor).
The `examples/` corpus and `scripts/` live at the workspace root.

```bash
cargo build --workspace                     # build everything
cargo run -p oxygrace-cli -- examples/axes.agr      # render to examples/axes.png
cargo run -p oxygrace-cli -- in.agr -o out.png --width 1466 --height 1076
cargo test --workspace                      # unit + corpus smoke + round-trip tests
cargo clippy --workspace                    # lint (keep clean)
cargo run -p oxygrace-gui [file]            # the GUI editor
# GUI self-screenshot (debug/CI): OXYGRACE_GUI_SHOT=/tmp/shot.png cargo run -p oxygrace-gui f.agr
# Web build (from oxygrace-gui/): trunk serve --release   (or trunk build --release → dist/)
#   wasm SIMD128 comes from .cargo/config.toml; CI deploy: .github/workflows/web-demo.yml
```

## Constraints (do not break these)

- **Pure Rust only**, no C library wrappers unless absolutely necessary.
- Command-language parsing uses the **`peg`** crate (`oxygrace/src/parse/grammar.rs`).
- Rendering stack: **`tiny-skia`** (rasterizer) + **`ttf-parser`** (glyph
  outlines) + **`png`** (output). All antialiased. The canvas also has an
  **SVG backend** (`oxygrace/src/render/svg.rs`): the same device-space paths are
  serialized as SVG, text as glyph outline paths — keep both backends fed
  by the shared geometry in `canvas.rs`, never add backend-specific
  geometry.
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
`mod.rs`**. (`oxygrace/src/model.rs` + `oxygrace/src/model/enums.rs`, etc.)

```
oxygrace/src/
  lib.rs             public API: load / load_str / render_png / render_svg /
                     render_pixmap (RenderResult: pixmap + RenderInfo) /
                     save_str / save
  model.rs + model/  plot data model (Project→Graph→Set, Axis, Frame, enums, defaults)
  color.rs           default 16-color map + @map color overrides
  font.rs            embedded URW base35 OTFs, glyph outlines via ttf-parser
                     (memoized: Mutex<HashMap> glyph cache, Arc<GlyphOutline>)
  text.rs            Grace string markup parser + glyph layout
  parse.rs + parse/  grammar.rs (peg), reader.rs (line loop + apply), data.rs (rows)
  write.rs           .agr writer (inverse of reader's apply; emits @version 50122)
  import.rs          plain-data import (-xy/-nxy/-type) + autoscale_world
  render.rs + render/ canvas.rs (shared device primitives + raster backend),
                     svg.rs (SVG backend), transform.rs (coords + inverses),
                     record.rs (hit-test side-channel: ElementId, RenderInfo —
                     a pure observer; pixel output identical with it on/off)
  draw.rs + draw/    plot.rs (draw order), axes.rs (ticks/labels), sets.rs (data)
                     — draw code tags elements via canvas.push/pop_element
oxygrace/assets/fonts/  bundled URW base35 OTFs (embedded via include_bytes!)
oxygrace/tests/      integration.rs (grammar/transform/corpus), hittest.rs
                     (recording purity + hit-test), roundtrip.rs (save_str
                     stability + render equality), stress.rs (1M-pt bench,
                     ignored) — corpus paths are ../examples
oxygrace-cli/src/    main.rs — the headless `oxygrace` binary (clap)
oxygrace-gui/src/    egui app: app.rs (state + panels), render.rs (dirty →
                     texture), plot_view.rs (ViewMap fit/blit, hit-test,
                     drags), inspector/ (property pages + rows), tree.rs,
                     file.rs (all dialog calls; cfg-split native/web),
                     theme.rs, args.rs, edit.rs, undo.rs
examples/            *.agr test corpus (from QtGrace6), at the workspace root
```

## Reference formulas (ported from QtGrace6 — cite the source in code)

These are the placement/geometry formulas we've reverse-engineered so far. Add
to this list as you port more; always keep the source reference.

- **View→device** is **isotropic**: `px = vx·side`, `py = page_h − vy·side`,
  `side = min(page_w, page_h)`; origin bottom-left in view space, Y flipped for
  the image (`rstdrv.cpp` `VPoint2gdPoint`, `page_scale = MIN2(w,h)`).
  `oxygrace/src/render/transform.rs`.
- **Default page**: our default is US-Letter landscape **792×612 px @ 72 DPI**.
  Files with `@page size W H` override it. (`DEFAULT_PAGE_WIDTH/HEIGHT` 733×538
  is Grace's *screen* default, not the hardcopy default.)
- **Old-format viewport rescale** (`graphs.cpp` `postprocess_project`): for
  `@version < 40005` force 792×612; for `version ≤ 40102` multiply every
  viewport (and view-loctype legend/object coords) by `get_page_viewport()` =
  `(width/side, height/side)` — pre-4.1.02 files store viewports as
  normalized-device-coords and must be stretched into the isotropic system.
  `oxygrace/src/parse/reader.rs` `postprocess_version`.
- **Line width px** = `linew · 0.0015 · side` (`MAGIC_LINEW_SCALE`, `globals.h`).
- **Font em px** = `charsize · 0.028 · side` (`MAGIC_FONT_SCALE`, `t1fonts.h`).
- **Tick mark length** = `0.02 · size` view units (`drawticks.cpp` `tsize`).
- **Tick label gap** `tl_offset = 0.01` view units (auto). Tick labels sit at
  `tl_offset` from the axis for inward ticks; `tsize + tl_offset` for outward
  (`drawticks.cpp` `vbase_tlabel`). x labels CENTER|TOP, y labels RIGHT|MIDDLE.
- **Axis label** anchor = `(distance to tick-label bbox edge) + tl_offset`
  (`drawticks.cpp` `vp_label_offset`). x label TOP-justified, y label
  MIDDLE-justified (rotated, centered). `oxygrace/src/draw/axes.rs`.
- **Symbol radius / bar half-width** = `0.01 · symsize` view units
  (`plotone.cpp` `drawxysym`, `drawsetbars`).
- **Chart bar grouping offset** accumulates `0.5·0.02·symsize` per set plus
  `bargap` (`plotone.cpp`); stacked charts accumulate y per category.
- **Font slot order** (no `@map font`) is Grace's t1lib order: 0 Times-Roman,
  **1 Bold, 2 Italic**, 3 BoldItalic, 4 Helvetica, 5 Helv-Bold, 6 Helv-Oblique,
  7 Helv-BoldOblique, 8 Courier… 12 Symbol, 13 ZapfDingbats. Verified against
  QtGrace's render of `tfonts.agr`. `oxygrace/src/font.rs`.
- **Default 16-color map** verbatim from `draw.cpp` `cmap_init`. `oxygrace/src/color.rs`.
- **Nine line-style dash patterns** and **32 fill patterns** copied from
  `patterns.h`. `oxygrace/src/render/canvas.rs`, `oxygrace/src/patterns.rs`.
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
  `oxygrace/src/font.rs` FONT_MAP_*.
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
  geographic (degreeslon/lat) and date/time formats (oxygrace/src/dates.rs,
  Julian-date conversions).

**M4 (done):**
- Error bars (all xy d* types, riser clip arrows); full markup engine
  (\v \h \z \c upperset, under/overline, marks, Symbol Greek);
  version-aware font mapping (+ `@map font`); SYM_CHAR symbols; avalue
  point labels; stacked-chart refy for all elements; `Fixed` graph
  viewport; per-point xycolor/xysize.
- hilo / boxplot / xyr-circle / vmap-arrow set renderers; pie and polar
  graph types (polar world->view in `transform.rs`, arc bars/grids in
  `axes.rs`); segment2/3 line types; `symbol skip`; opaque pattern
  fills (fg-on-bg, like the gd driver).
- Not implemented: smith charts (grace's own `draw_smith_chart` is an
  empty stub — there is no reference behavior); CSV import. `.xvg`
  files load through the tolerant reader.

**Post-M4:** SVG output (`render_svg`, `-o out.svg`): the canvas grew a
backend enum — raster (tiny-skia) and SVG writer — both fed identical
device-space geometry, so the SVG matches the PNG pixel-for-pixel
(validated by rasterizing with Chromium and diffing). Text is emitted
as glyph outline paths.

**Alpha channels (QtGrace extension):** per-set pen opacities ride in
`#QTGRACE_ADDITIONAL_PARAMETER: G n S m ALPHA_CHANNELS
{line;fill;sym;symfill;avalue;errbar}` comment lines (QtGrace files.cpp;
0..=255, default 255). Parsed in `reader.rs` (existing sets only, like
`is_valid_setno`), stored on `Pen.alpha` / `AValue.alpha` / `ErrBar.alpha`,
applied through the canvas `set_alpha` state (QtGrace `draw.cpp setalpha`;
the Qt driver stamps `col.setAlpha(getalpha())`) in both raster and SVG
backends (`stroke-opacity`/`fill-opacity`), written back only when
non-default so plain Grace files stay byte-identical. Translucent fills
don't occlude in hit-testing. GUI: opacity slider inside the six set
color pickers (`rows::color_alpha`). Graph/axis/object-level QtGrace
alphas (`GRAPH_ALPHA`, `AXIS_ALPHA`, …) are still ignored.

**GUI Phase 0 + G1 (done):** cargo workspace; `render_pixmap` (raw
premultiplied-RGBA + `RenderInfo` hit-test geometry, recorded as a pure
side-channel on the canvas — guarded by a corpus pixel-equality test);
`.agr` writer (`save_str`/`save`, validated by save-stability + render-
equality round-trip tests over the whole corpus and by opening saved files
in QtGrace); glyph-outline cache (warm full render 2–12 ms in release);
`oxygrace-gui` viewer — open via rfd, dirty-flag texture loop, zoom-to-fit
letterboxed canvas, click → hit-test in the status bar, panel skeleton.

**GUI G2 (done):** click/hover selection on the canvas (hit-test + Esc
clears), selection overlay (bounds box + 8 handles) and hover highlight
painted on top of the texture (no re-render), project tree in the left
panel sharing the `ElementId` selection currency (hidden items grayed),
`tree::describe` for status/inspector text, `OXYGRACE_GUI_SELECT=set:0:1`
debug hook for screenshot tests, corpus self-consistency test
(every visible graph/set records bounds). High-contrast theme
(`theme.rs`: zoom 1.25, bright text, halo-outlined selection overlay) —
hardcoded for now, to become configurable.

**GUI G3 (done):** the editor MVP. `edit.rs` command layer (widgets queue
`Edit { label, coalesce, apply: Box<dyn FnOnce(&mut Project)> }`; the app
applies them after the UI pass — widgets never mutate the model);
`undo.rs` snapshot stack (limit 50, slider drags / typing coalesce by
label into one step, Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y, Edit menu shows step
labels); `inspector/rows.rs` property-row vocabulary (num/int/text/toggle/
combo/color-with-swatches/font/linestyle-with-dash-preview, all in
2-column grids) and pages: page, graph (world/viewport/scales/titles),
axis (bar/ticks/tick-labels/label sections, clicked sub-element opens
expanded), set (line/symbol/fill/errorbar/avalue), legend, frame, objects
(string/line/box/ellipse/timestamp); Save / Save As (Ctrl+S, rfd) via the
core writer, modified flag in the window title, confirm-on-close modal.
UX refinements: sub-elements (title, tick labels…) share their parent's
page with the clicked section force-expanded (App.refocus flag); shape-based
selection highlight via `RenderInfo::shapes_of` (clip-aware); `hit_candidates`
+ same-spot click cycling reaches occluded elements — candidates are *scored*
(visible-over-occluded, then ink > fill > click-region, then distance to ink,
then smaller ink area, then draw order), so an exact hit on a curve beats a
nearby fill edge, an axis edge beats the coincident frame outline, and the
plot-area region never shadows drawn ink; opaque fills occlude ink below
them (demoted, still cyclable); grid lines muted from recording entirely;
swatch-grid color &
pattern pickers; −/+ spin buttons; tree scrolls both ways so panels shrink
freely; frame merged into the plot-area page.

**GUI G4 (done):** direct manipulation. Core grew the inverse transforms
(`PageTransform::device_to_view`, `WorldTransform::view_to_x/y/world`, incl.
polar/fixed/invert — round-trip tested). Canvas drags (plot_view.rs):
drag-move legend, strings, lines, boxes, ellipses, timestamp (view- or
world-anchored, converted through the owning graph's transform); drag the
8 selection handles of a selected plot area to resize the viewport, drag
inside to move it. Each gesture coalesces into one undo step
(`App::end_gesture` on release); cursor feedback (grab/resize icons);
recomputed from press-time originals each frame so there is no drift.
**GUI G4.5 (done):** xmgrace-style CLI (`args.rs`: project/data files,
`-xy`, `-nxy`, `-type`, `-free`; core `oxygrace/src/import.rs` `import_data_str` +
`autoscale_world`); free page aspect (View → Free aspect: page follows the
canvas AND viewports/view-anchored objects rescale with the page extents —
the postprocess_version stretch — so the plot fills the window; original
geometry restored on toggle-off, and Save writes the un-stretched
geometry — free aspect is a view mode, not an edit); dark/light modes (View →
Mode, `theme::apply(ctx, Mode)`); status bar updates on hover (element +
overlap count); menus switch on hover once one is open; frame edges are
recorded as axis ink (`record_polyline_view`) so axes are selectable when
bars are off (au.agr); highlight polylines dedup consecutive points (egui
tessellator spike artifacts); line annotations get draggable endpoint
handles; rotatable elements (strings, timestamp) arm rotate mode on second
click — corner circles, drag rotates around the anchor.
**Perf (1M-point stress, tests/stress.rs — run with `cargo test --release
--test stress -- --ignored --nocapture`):** dense solid polylines (>4096
device points) are M4-decimated per device x-column (first/min/max/last —
pixel-equivalent for thin strokes; dashed lines exempt) in the shared
geometry path, and dense uniform symbol clouds (>4096) dedup by
half-radius cells (per-point size/color and Char symbols exempt). 1M-point
line: ~1.5 s → 25–50 ms render, hover hit-test µs; 1M symbols: 8.2 s →
0.3 s, records 2M → 53k. Theme: View → Mode has System/Dark/Light —
egui's `system_theme()` gives the OS dark/light preference only (no system
palette colors exist in egui).
**GUI G5 (done):** wasm web build. `main.rs` cfg-split (native
eframe::run_native vs eframe::WebRunner onto `oxygrace_canvas` in
index.html); `file.rs` cfg-split — web opens via `rfd::AsyncFileDialog`
(results delivered through `App::file_tx` mpsc, polled in `logic`) and
saves by triggering a browser download (web-sys Blob + anchor); wasm
SIMD128 enabled via `.cargo/config.toml`; the web demo boots with a
bundled example. Bundle via trunk (`oxygrace-gui/index.html`,
`Trunk.toml`, ~8.4 MB wasm); GitHub Pages deploy workflow in
`.github/workflows/web-demo.yml` (enable Pages with source "GitHub
Actions"). Verified end-to-end in headless Chromium: UI + plot render in
the browser, System theme follows `prefers-color-scheme`.

**GUI toolbar (done):** an icon-only toolbar (`icons.rs` — monochrome
glyphs drawn procedurally with the egui painter, no image assets) above the
project tree, wrapping to the panel width: Open, Save, Autoscale-all,
Autoscale-to-set, Pan, Free aspect. Two modal canvas tools (`App::tool`:
Select/Pan/PickSet, gated in `plot_view::show`): Pan drags the world window
of the graph under the cursor (`WorldTransform::pan_world`, scaled-space so
log axes pan uniformly), Autoscale-to-set fits a graph to a clicked set.
Autoscale helpers set the world to data extents (all non-hidden sets, or
one); all are undoable edits.

**Wayland IME workaround** (`defuse_broken_ime` at the top of `App::ui`,
Linux-gated): recent Wayland compositors make winit stream `Ime(Disabled)` +
deliver typed chars as `Ime(Commit(..))` with no `Enabled`/`Preedit`, which
egui 0.34.3 mishandles so text fields accept only the **first** character
(paste/backspace still work). We rewrite `Ime(Commit)`→`Text` and drop stray
`Ime` events. No-op on X11; macOS/Windows/wasm untouched. See
`mod ime_workaround_tests` in `oxygrace-gui/src/app.rs`. (Same fix shipped in
molar_vis.)

GUI milestones complete (G1–G5); roadmap history in
`/home/semen/.claude/plans/polished-coalescing-biscuit.md`; toolkit
analysis in `docs/gui-analysis.md`.

See `/home/semen/.claude/plans/we-will-build-an-wobbly-elephant.md` for the
original plan.
