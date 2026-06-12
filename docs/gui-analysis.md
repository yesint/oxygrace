# Oxygrace GUI: toolkit analysis and design proposal

*Research date: June 2026. Sources: survey of toolkit repos/changelogs/release
pages as of 2026-06-12, plus an embeddability audit of the oxygrace core.
Goal: a full-scale interactive editor GUI (Origin-style selectable plot
elements, standardized property inspectors docked as a sidebar — no floating
dialogs), pure Rust preferred, file-format compatibility with Grace preserved,
no GUI-layout compatibility with Grace.*

---

## 1. Recommendation up front

**Build the GUI natively in Rust with egui** (`eframe` + `egui_tiles` or
`egui_dock` for panels + `egui_extras` for tables + `rfd` for file dialogs),
with the plot shown as a texture blitted from the existing tiny-skia pixmap and
**hit-testing done geometrically in the oxygrace core**, not in the toolkit.

This is the only option that simultaneously gives:

- a production-proven precedent of *exactly this app shape* — **Rerun**
  (docked panes + custom rendered viewport + property inspectors, native and
  in-browser, built and funded on egui);
- the best-documented path for embedding a CPU-rasterized RGBA image with
  mouse interaction (`TextureHandle::set()` + `Sense::click_and_drag()`);
- a credible **"same app runs in the browser" wasm story essentially for
  free** (with caveats, §7);
- MIT/Apache licensing, mandatory AccessKit accessibility, and the healthiest
  funding alignment in the ecosystem (the maintainer is CTO of Rerun;
  Rerun raised $17M in 2025).

The serious alternatives and why they lost are in §4; the browser-based
variant is analyzed in §5 — it is viable (Tauri 2 + TypeScript UI, the
Graphite blueprint) and would win *if the browser were the primary delivery
vehicle*, but costs a second language/stack and inherits Linux WebKitGTK
webview fragility.

---

## 2. What the core gives us today, and what it needs first

An audit of the current codebase (≈7.5 KLOC) found it well-shaped for
embedding: the model (`Project → Graph → Set`, `Axis`, `Frame`) is plain
mutable data with public fields and no interior mutability — a GUI can edit a
field and re-render; there is no global state; fonts are embedded
(one-time ~10 ms load); file I/O happens only in `load()`; the canvas already
abstracts two backends (raster + SVG) fed by identical device-space geometry.

Required core work, **toolkit-independent** (do this before any GUI code):

1. **Expose the raw pixmap.** `render_png` returns encoded PNG bytes. Add
   `render_pixmap(&Project) -> Pixmap` (or RGBA8 buffer + dimensions) so the
   GUI blits without an encode/decode round-trip. Trivial — the pixmap
   already exists inside the raster backend. Note tiny-skia pixmaps are
   **premultiplied alpha**; egui's `ColorImage` has a matching constructor.
2. **An `.agr` writer.** Parser only today. Serializing the model back to the
   Grace command language (~500–1000 lines) is mandatory for an editor and
   also independently useful headlessly. Round-trip tests against the
   `examples/` corpus (`load → save → load` model equality) come free.
3. **Hit-testing.** Drawing code currently doesn't know which model element
   it draws. The clean fix exploits the existing backend abstraction: add a
   **pick/record pass** — drawing routines tag geometry with an element ID
   (`ElementId::Set(g, s)`, `Axis(g, which)`, `Legend(g)`, `Title`,
   `AnnotationString(i)`, …) and a recording backend stores
   `(id, device-space bounding shape)`. Hit-testing is then bbox pre-filter +
   per-shape math in the core (the Excalidraw/Graphite pattern — every
   serious editor surveyed does geometric hit-testing against the scene
   model; nobody uses picking buffers for 2D). The toolkit only ever sends
   the core a cursor position and gets back an `ElementId`.
4. **Interactive render performance.** Full re-render is 50–200 ms, glyph
   rasterization dominating. Fine for property edits (re-render on commit),
   marginal for live dragging. Levers, in order of value: a glyph-outline
   cache in `font.rs` (biggest win, simple); render-on-edit only (egui's
   reactive repaint means the texture is re-uploaded only when the model
   changes); optional half-resolution render during drag gestures. Do **not**
   build incremental/dirty-region rendering up front — measure first.

---

## 3. Retained vs immediate mode — the honest analysis

The prior "retained is probably preferable" is reasonable in the abstract but
**does not survive contact with the 2026 Rust ecosystem for this particular
app**. Three findings:

**(a) The classic objections to immediate mode are mostly resolved or
inapplicable here.** The idle-CPU/battery critique targets continuous
repainting; egui's default reactive mode repaints only on input or explicit
request — "if your app is idle, no CPU is wasted". A plot editor re-renders
the expensive thing (the tiny-skia pixmap) only on model edits regardless of
UI paradigm. The remaining real immediate-mode costs are: single-pass layout
(centering/size-to-content needs egui's sizing-pass workaround), per-frame
relayout of very large scroll areas, and "logic creep" (UI and model code
entangling) — the last is a discipline issue, addressed in §6 by keeping all
mutation behind a command layer.

**(b) Immediate mode is a *positively good* fit for this model.** Oxygrace's
document is plain mutable data. In egui, an inspector row is literally
`ui.add(DragValue::new(&mut axis.tick_major))` — no message enum, no diffing,
no binding DSL. In iced (Elm) every field edit is a `Message` variant plus a
match arm: real, mechanical boilerplate multiplied by the hundreds of
properties a Grace editor exposes. In Slint, every property crosses a Rust ↔
`.slint` DSL boundary. For a forms-heavy inspector over a large existing Rust
struct hierarchy, immediate mode minimizes the code between the model and the
widgets.

**(c) The retained options carry concrete gaps.** Slint: no docking, no color
picker, weak wasm ("demonstration purposes"), a second language, and a
royalty-free license tier with *mandatory attribution* (or GPLv3 — fine for
us as MIT-incompatible only in spirit, but the attribution/licence text is
something to re-read at adoption). iced: 15-month release gaps until 0.14
(Dec 2025), AccessKit still unmerged, wasm second-tier, an open canvas-cache
regression with large images. Notably, Tritium (safety-critical tooling)
attempted an egui→Slint migration in 2025–26 and **abandoned it** as an
"unforced error", refactoring within egui instead.

**Verdict:** choose egui not "despite" immediate mode but partly because of
it. If a retained toolkit is strongly preferred anyway, **Slint** is the
right one (LibrePCB 2.x — a Qt-app-rewritten-in-Slint technical editor with a
custom canvas — is a near-identical precedent, and Slint has the best
accessibility and native menus), accepting hand-rolled docking, a hand-rolled
color picker, the DSL split, and no real web target.

---

## 4. Native toolkit comparison (state of mid-2026)

| | **egui 0.34** | iced 0.14 | Slint 1.16 | GPUI + gpui-component | Xilem 0.4 | Floem |
|---|---|---|---|---|---|---|
| Paradigm | immediate (reactive repaint) | retained, Elm | retained, declarative DSL | hybrid (per-frame elements over entity state) | retained, ECS-ish | retained, signals |
| RGBA-pixmap canvas + mouse | **best-documented pattern** | OK (canvas paths better than blit; open large-image cache bug #3173) | proven (LibrePCB), worker-thread pattern needed | exists, API unverified | best architecture, alpha | OK (has tiny-skia fallback) |
| Docking / panels | egui_tiles (Rerun) / egui_dock | pane_grid (no tear-off/tabs) | DIY | **real docking built in** | none | none |
| Color picker, combos, tables | all built-in / egui_extras | iced_aw + new table widget | no picker; table yes | **~60 components incl. picker, virtualized table** | almost nothing | partial |
| Native menus | no (muda workaround) | no | **yes (macOS/Win)** | yes | no | partial |
| Same-app wasm | **yes, first-class** | demo-tier | demo-tier | no | early | unverified |
| Accessibility | good (AccessKit mandatory since 0.34) | **missing** (AccessKit not merged) | **best in class** | weak | wired, early | weak |
| License | MIT/Apache | MIT | GPLv3 / royalty-free w/ mandatory attribution / paid | Apache-2.0 | Apache-2.0 | MIT |
| Production proof | **Rerun** (this exact app shape) | COSMIC desktop, Halloy | **LibrePCB 2.x** (this exact app shape) | Zed, Longbridge Pro | none | Lapce (cooling) |
| Momentum risk | low (Rerun-funded) | slow cadence, one lead | small company | serves Zed first, pre-1.0 churn, rough Windows | funding in flux post-Google | medium |

Ranking for this project: **1. egui** — precedent + canvas pattern + wasm +
license + momentum. **2. Slint** — if native menus/a11y/retained outweigh
docking+picker DIY and the license terms. **3. GPUI + gpui-component** —
the richest single widget kit (docking, tables, color picker in one coherent
Apache-2.0 kit, $32M-funded) but pre-1.0, sparse docs, no wasm, weak a11y;
a wildcard worth re-checking in a year. **4. iced** — credible (COSMIC!) but
Elm boilerplate for hundreds of properties, no a11y, weak wasm. **Xilem** is
the architecture to re-evaluate in 2027–28; **Floem/Makepad/Dioxus-native
(Blitz)** are not advised now (Blitz still alpha, targeting production
"sometime in 2026").

No mature Origin/Grace-like native Rust plotting editor exists — the niche is
open.

---

## 5. The browser-based alternative

Architecture that the research converged on (and that every surveyed editor —
Figma, Graphite, Excalidraw, Vega — independently uses): **one engine owns a
retained scene model; hit-testing is geometric against that model; property
panels are a separate declarative UI layer talking to the engine via
messages.** [Graphite](https://graphite.art) is the blueprint: Rust core +
thin Svelte/TS frontend over a coarse-grained message protocol.

Concrete findings:

- **Rendering**: blitting the tiny-skia pixmap to `<canvas>` via `ImageData`
  over a wasm-memory view costs ~1–3 ms/frame at 1500×1100 (estimate — no
  published benchmark; prototype first). tiny-skia has real wasm SIMD128
  (build with `-C target-feature=+simd128`, ~93% browser baseline). Opaque
  plot background sidesteps the premultiplied-vs-straight alpha mismatch.
  resvg (same tiny-skia + ttf-parser stack) already ships as a wasm package —
  the stack provably runs in browsers.
- **SVG-DOM rendering** (our SVG backend + browser hit-testing for free) is
  viable to ~5–10k DOM nodes if each set collapses to one `<path>`; it's a
  nice option for small plots but not the thing to bet the editor on, and our
  text-as-glyph-outlines multiplies nodes. Hit-testing should live in the
  Rust core anyway (§2.3), which makes this moot.
- **UI layer**: TypeScript (Svelte 5 or React 19 + shadcn/ui) is the
  lower-risk choice — the component ecosystem (accessible comboboxes, color
  pickers, resizable panels) is years ahead of Rust-web equivalents. Of the
  full-Rust web frameworks only **Dioxus** (0.7, monthly releases, 28
  accessible components) and **Leptos** (0.8.x, smallest bundles) are
  healthy; Yew just exited a 26-month stall; Sycamore is dormant.
- **Desktop packaging**: **Tauri 2.x**, with the killer detail that the
  backend is Rust — *the oxygrace core links natively, no wasm needed for the
  desktop app at all*; real file dialogs/menus, 3–15 MB bundles. The risk is
  webview fragmentation: WebKitGTK on Linux (our own platform, Fedora) has
  documented font-weight and compositing bugs and a maintainer-acknowledged
  quality problem — mitigated somewhat because the plot is rendered by our
  rasterizer, not by webview CSS, so the risk is contained to UI chrome.
  Electron (+ napi-rs native addon) is the heavyweight fallback. **PWA +
  File System Access API is ruled out**: `showOpenFilePicker` remains
  Chromium-only; Firefox's position is "harmful". Scientists need in-place
  file save.

**Verdict:** a real, well-trodden architecture — choose it if the browser is
the primary target or if a polished web-grade widget ecosystem matters more
than a single language. The costs are permanent: two stacks, a serialized
message boundary to maintain, webview QA on Linux, and the loss of the
pure-Rust character of the project. Given that the desktop is the primary
target and wasm is a "nice if cheap" goal, **the native egui app wins** — and
it still produces a browser build (§7), just from one codebase.

---

## 6. Proposed architecture and design decisions

**Crate layout** — workspace, GUI strictly above the library:

```
oxygrace/        existing library (rendering core, parser, writer, hit-test)
oxygrace-gui/    egui app: shell, panels, inspector widgets, undo, commands
```

The core API grows: `render_pixmap`, `save_str(&Project) -> String`,
`hit_test(&Project, &RenderInfo, px, py) -> Option<ElementId>`,
`element_bounds(...) -> Shape` (for selection handles). `RenderInfo` is the
recorded ID→geometry table produced as a side product of a render pass.

**Selection model (Origin-style).** Click → core hit-test → `ElementId`
becomes the app's `selection`. Selection handles/highlight are drawn by the
GUI as an **overlay** with egui's painter on top of the texture — never baked
into the pixmap — so hover/selection feedback costs no re-render. Double-click
jumps the inspector to the element; Esc clears; future: rubber-band via
rect-vs-bounds math in the core.

**Inspector (sidebar, standardized — the user's key design requirement).**
One docked right-side panel whose content is dispatched on `ElementId`:
graph/axis/set/legend/annotation pages built from a small vocabulary of
reusable property-row widgets (`prop_color`, `prop_linew`, `prop_font`,
`prop_combo<T: GraceEnum>`, …) so every page looks and behaves identically.
*What* to expose is scraped from QtGrace's dialogs (Plot/Set/Axis/Graph
appearance), not *how* — regroup freely. Left side: a project tree
(graphs → sets, annotations) as an alternative selection path, egui_tiles
making the layout user-rearrangeable.

**Mutation, undo, redraw.** All edits go through a command layer:
`app.apply(Edit)` mutates the `Project`, pushes undo state, and marks the
render dirty. The model is plain `Clone`-able data of trivial size — **undo =
snapshot stack** (clone on edit boundary, coalesce slider drags), no command
inverses needed. This also walls off "logic creep": widgets produce `Edit`s,
they never mutate the model directly. Dirty flag → re-render →
`TextureHandle::set` — egui repaints only then.

**File round-trip.** Open/save via `rfd`; saving uses the new writer, so files
remain forward-compatible with Grace/QtGrace — the format *is* the project
file, no new format invented.

**Known egui gaps to plan around:** no native macOS menu bar (use `muda` or
an in-window egui menu bar — acceptable given "modern, no Grace
compatibility"); undo/redo is ours (by design, see above); a fancier color
picker than the built-in may eventually be wanted.

---

## 7. The wasm story

With egui/eframe the same app compiles to a browser build essentially for
free: wgpu with WebGL fallback, proven by Rerun shipping its viewer on the
web. Build the core with `+simd128`. Honest caveats, from eframe's own README:
web text editing/IME is the weak spot (hidden-input hacks), screen-reader
support on web is experimental, and content isn't browser-searchable. For a
"try it in the browser" channel and demos this is perfect; if the web version
ever becomes the *primary* product, that's the point to revisit the
Graphite-style two-stack architecture (§5) — the core API designed in §6
(commands in, scene/bounds out) is exactly the message protocol that
architecture would need, so nothing is wasted.

---

## 8. Risks and prototype spikes (do these first, ~days not weeks)

1. **Blit + interact spike:** eframe window, render an example to pixmap,
   `TextureHandle::set`, click → log cursor in plot coordinates. Validates
   the entire rendering/interaction premise.
2. **Glyph cache:** measure render time before/after caching glyph outlines;
   target <30 ms re-render for typical corpus files.
3. **Hit-test recording pass:** thread `ElementId` through `draw/` for one
   element class (sets), prove the recording-backend design.
4. **Writer round-trip:** serialize one graph's worth of state, reload,
   compare models; check the file opens in QtGrace.
5. Re-evaluate in 12 months: GPUI/gpui-component maturity, Xilem widgets,
   Blitz — the decision above is right for 2026, not forever; the §6
   command-layer architecture keeps the toolkit swappable.

## 9. Suggested GUI milestones

- **G1 — viewer:** open/render/zoom-fit, texture blit, panel skeleton
  (egui_tiles), file dialog. *(spike 1 grown up)*
- **G2 — selection:** hit-test pass in core, click-to-select + overlay
  handles, project tree.
- **G3 — inspector:** property-row vocabulary; axis/set/graph/legend pages;
  command layer + snapshot undo; `.agr` writer + Save/Save As.
- **G4 — direct manipulation:** drag annotations/legend, drag-resize
  viewport, double-click conveniences; glyph cache / drag-time perf.
- **G5 — web build:** wasm target, deploy demo; CI artifact.
