//! `.oxgr` native format tests: hand-written documents parse to the right
//! model, the covered slice round-trips render-identically, unknown fields
//! from future revisions are tolerated, and heredoc data is collision-proof.

use std::sync::Arc;

use oxygrace::format::{load_oxgr_str, save_oxgr_str};
use oxygrace::model::{Defaults, FillType, LineType, Set, SymbolType, World};
use oxygrace::Project;

/// A hand-authored document exercising the covered slice.
#[test]
fn sample_document_parses() {
    let doc = r###"
// hand-written sample
(
    format: 1,
    page: (width: 400, height: 300),
    colors: [(id: 2, rgb: "#2171b5")],
    graphs: [(
        world: (xmin: 0.0, xmax: 10.0, ymin: -1.0, ymax: 1.0),
        view: (xmin: 0.15, xmax: 1.15, ymin: 0.15, ymax: 0.85),
        title: (text: "Sample", size: 1.5, font: 0, color: 1),
        // Partially-written sub-structs: omitted fields fall back to the
        // baselines (struct-level serde defaults).
        axes: (
            y: (
                active: true,
                label: (text: "amplitude"),
                ticks: (major: 0.5, side: Opposite),
                tick_labels: (side: Opposite),
            ),
        ),
        sets: [(
            legend: "wave",
            line: (kind: Straight, dash: Dotted, width: 2.0, color: 2, opacity: 100),
            fill: (kind: Baseline, rule: 0, color: 2, pattern: 1, opacity: 80, baseline: 0),
            data: r#"
0 0.1
1 0.5  "peak"
2 -0.3
"#,
        )],
    )],
)
"###;
    let p = load_oxgr_str(doc).expect("sample parses");
    assert_eq!((p.page_width, p.page_height), (400, 300));
    assert_eq!(p.color_overrides, vec![(2, (0x21, 0x71, 0xb5))]);
    let g = &p.graphs[0];
    assert_eq!(g.world.xmax, 10.0);
    assert_eq!(g.labels.title, "Sample");
    // The configured y axis moved to the opposite side…
    assert_eq!(g.axes[1].tl_op, 1);
    assert_eq!(g.axes[1].label, "amplitude");
    // …while omitted axes keep their slot baselines (alt axes inactive).
    assert!(g.axes[0].active);
    assert!(!g.axes[2].active);
    let s = &g.sets[0];
    assert_eq!(s.legend, "wave");
    assert_eq!(s.linestyle, 2, "dash name maps to Grace style index");
    assert_eq!(s.line_pen.alpha, 100);
    assert_eq!(s.fill_type, FillType::Baseline);
    assert_eq!(s.fill_pen.alpha, 80);
    assert_eq!(s.data.x().unwrap(), &[0.0, 1.0, 2.0]);
    assert_eq!(s.data.y().unwrap(), &[0.1, 0.5, -0.3]);
    assert_eq!(s.data.strs[1].as_deref(), Some("peak"));
}

/// Build a project from the covered slice, save → load → the render is
/// byte-identical (the strongest equality we have).
#[test]
fn round_trip_preserves_render() {
    let mut p = Project::default();
    p.page_width = 500;
    p.page_height = 400;
    p.color_overrides.push((5, (10, 200, 90)));
    let g = p.graph_mut(0);
    g.world = World { xmin: 0.0, xmax: 6.5, ymin: -1.5, ymax: 1.5 };
    g.labels.title = "Round trip".into();
    g.labels.subtitle = "slice".into();
    g.axes[0].label = "x".into();
    g.axes[0].major = 1.0;
    g.axes[1].label = "sin x".into();
    g.axes[1].major = 0.5;
    g.axes[1].op = 1;
    g.axes[1].tl_op = 1;

    let defaults = Defaults::default();
    let mut wave = Set::with_defaults(&defaults);
    let xs: Vec<f64> = (0..80).map(|i| i as f64 * 6.5 / 79.0).collect();
    let ys: Vec<f64> = xs.iter().map(|x| x.sin()).collect();
    Arc::make_mut(&mut wave.data).cols = vec![xs.clone(), ys];
    wave.legend = "sine".into();
    wave.line_pen.color = 2;
    wave.linewidth = 2.0;
    wave.fill_type = FillType::Baseline;
    wave.fill_pen.color = 5;
    wave.fill_pen.alpha = 90; // translucent fill exercises the opacity path
    g.sets.push(wave);

    let mut dots = Set::with_defaults(&defaults);
    let ys2: Vec<f64> = xs.iter().map(|x| (x * 0.7).cos()).collect();
    Arc::make_mut(&mut dots.data).cols = vec![xs, ys2];
    dots.line_type = LineType::None;
    dots.symbol = SymbolType::Circle;
    dots.symbol_size = 0.6;
    dots.symbol_pen.color = 4;
    dots.symbol_fill.color = 5;
    dots.symbol_fill.pattern = 1;
    dots.symbol_fill.alpha = 120;
    dots.symskip = 3;
    g.sets.push(dots);

    let text = save_oxgr_str(&p);
    // The data must be embedded as readable literal rows (plain multiline
    // string, or a raw string when the content needs it) — never as a
    // single line of \n escapes.
    assert!(text.contains("\n0 0\n"), "data rows not literal:\n{text}");
    assert!(!text.contains("\\n"), "data was newline-escaped:\n{text}");
    let back = load_oxgr_str(&text).expect("round trip parses");
    assert_eq!(
        oxygrace::render_png(&p),
        oxygrace::render_png(&back),
        "render changed across .oxgr round trip"
    );
}

/// Fields from future revisions are skipped, not fatal (serde default).
#[test]
fn unknown_fields_are_tolerated() {
    let doc = r#"
(
    format: 1,
    frobnicator: 42,
    page: (width: 300, height: 200),
    graphs: [(
        world: (xmin: 0.0, xmax: 1.0, ymin: 0.0, ymax: 1.0),
        shiny_new_feature: (level: 11),
        sets: [(
            legend: "s",
            hologram: true,
        )],
    )],
)
"#;
    let p = load_oxgr_str(doc).expect("unknown fields tolerated");
    assert_eq!(p.page_width, 300);
    assert_eq!(p.graphs[0].sets[0].legend, "s");
}

/// Per-point labels containing `"#` force the writer to escalate the raw
/// string delimiter; the data must still round-trip exactly.
#[test]
fn heredoc_delimiter_collision_round_trips() {
    let mut p = Project::default();
    let g = p.graph_mut(0);
    let mut s = Set::with_defaults(&Defaults::default());
    {
        let d = Arc::make_mut(&mut s.data);
        d.cols = vec![vec![0.0, 1.0], vec![2.0, 3.0]];
        d.strs = vec![Some("tricky \"# label".into()), None];
    }
    s.avalue.active = true;
    s.avalue.avtype = 4; // per-point strings
    g.sets.push(s);

    let text = save_oxgr_str(&p);
    let back = load_oxgr_str(&text).expect("collision doc parses");
    let d = &back.graphs[0].sets[0].data;
    assert_eq!(d.cols, p.graphs[0].sets[0].data.cols);
    assert_eq!(d.strs[0].as_deref(), Some("tricky \"# label"));
}

/// Newer format revisions are refused with a clear error.
#[test]
fn future_version_is_refused() {
    let err = load_oxgr_str("(format: 2)").unwrap_err();
    assert!(err.to_string().contains("format 2"), "{err}");
}

/// The accountant: every corpus `.agr` must render byte-identically after
/// an `.agr → .oxgr → render` round trip. Any model field the renderer
/// consumes that the schema misses shows up here.
#[test]
fn corpus_renders_identically_through_oxgr() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "agr") {
            continue;
        }
        let project = oxygrace::load(&path).unwrap();
        let text = save_oxgr_str(&project);
        let back = load_oxgr_str(&text)
            .unwrap_or_else(|e| panic!("{path:?}: .oxgr reload failed: {e}"));
        assert_eq!(
            oxygrace::render_png(&project),
            oxygrace::render_png(&back),
            "render differs through .oxgr for {path:?}"
        );
        n += 1;
    }
    assert!(n > 0, "no corpus files found");
}
