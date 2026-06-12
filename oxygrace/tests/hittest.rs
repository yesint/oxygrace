//! Hit-test recording tests: the recorder must not perturb rendering, and
//! recorded geometry must answer "what is at this pixel?" sensibly.

use oxygrace::{ElementId, FontSet, Project};

/// View coordinates -> device pixels (the isotropic page mapping).
fn dev(project: &Project, vx: f64, vy: f64) -> (f32, f32) {
    let side = project.page_width.min(project.page_height) as f64;
    (
        (vx * side) as f32,
        (project.page_height as f64 - vy * side) as f32,
    )
}

/// Recording is a pure observer: PNG output is byte-identical with the
/// recorder on (render_pixmap) and off (render_png), across the corpus.
#[test]
fn recording_does_not_perturb_rendering() {
    let fonts = FontSet::load();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "agr") {
            continue;
        }
        let project = oxygrace::load(&path).unwrap();
        let plain = oxygrace::render_png(&project);
        let recorded = oxygrace::render_pixmap(&project, &fonts)
            .pixmap
            .encode_png()
            .unwrap();
        assert_eq!(plain, recorded, "pixel mismatch with recording on: {path:?}");
        n += 1;
    }
    assert!(n > 0, "no corpus files found");
}

/// Targeted hits against examples/axes.agr: view 0.30..0.65 x 0.35..0.70,
/// world -1..1 with zero-type alt axes crossing at the view center.
#[test]
fn hit_test_axes_example() {
    let fonts = FontSet::load();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/axes.agr");
    let project = oxygrace::load(path).unwrap();
    let res = oxygrace::render_pixmap(&project, &fonts);
    let info = &res.info;
    let hit = |vx: f64, vy: f64| {
        let (x, y) = dev(&project, vx, vy);
        info.hit_test(x, y, 3.0)
    };

    // Page margin: nothing there.
    assert_eq!(hit(0.05, 0.05), None);

    // Inside the viewport, away from any axis: the graph fallback region.
    assert_eq!(hit(0.36, 0.60), Some(ElementId::Graph(0)));

    // Frame edges double as axis lines: the bottom edge selects the x axis
    // (the frame itself is next in the click-cycle order)…
    assert_eq!(hit(0.40, 0.35), Some(ElementId::AxisBar { graph: 0, axis: 0 }));
    // …and the offset x-axis bar at 0.35 − 0.15 = 0.20 is the axis too.
    assert_eq!(hit(0.40, 0.20), Some(ElementId::AxisBar { graph: 0, axis: 0 }));

    // The zero-type alt axes cross at the view center: an axis bar wins
    // over the graph fallback there.
    assert!(matches!(
        hit(0.475, 0.525),
        Some(ElementId::AxisBar { graph: 0, .. })
    ));

    // Tick labels recorded their text boxes; the center of their union box
    // hits the labels themselves.
    let tl = ElementId::TickLabels { graph: 0, axis: 0 };
    let b = info.bounds(tl).expect("x tick labels recorded");
    assert_eq!(info.hit_test((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0, 1.0), Some(tl));

    // Selection bounds of the graph lie within the page.
    let g = info.bounds(ElementId::Graph(0)).unwrap();
    assert!(g.x0 >= 0.0 && g.y0 >= 0.0);
    assert!(g.x1 <= project.page_width as f32 && g.y1 <= project.page_height as f32);
}

/// Where an axis bar coincides with the frame border (the common case —
/// log2log.agr draws its axes on the frame edges), the axis must win the
/// hit even though the frame is drawn on top, or axes would be unclickable.
#[test]
fn axis_wins_over_coincident_frame() {
    let fonts = FontSet::load();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/log2log.agr");
    let project = oxygrace::load(path).unwrap();
    let res = oxygrace::render_pixmap(&project, &fonts);
    let v = project.graphs[0].view;
    let (x, y) = dev(&project, (v.xmin + v.xmax) / 2.0, v.ymin);
    assert_eq!(
        res.info.hit_test(x, y, 3.0),
        Some(ElementId::AxisBar { graph: 0, axis: 0 })
    );
}

/// Grid lines are decoration: hovering/clicking them must not hit the axis
/// (bar.agr draws y-grid lines across the whole plot).
#[test]
fn grid_lines_do_not_hit_axes() {
    let fonts = FontSet::load();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/bar.agr");
    let project = oxygrace::load(path).unwrap();
    let res = oxygrace::render_pixmap(&project, &fonts);
    let g = &project.graphs[0];
    assert!(g.axes.iter().any(|a| a.major_props.grid), "bar.agr should have grids");
    // Sweep the plot interior (away from the edges): no axis may hit.
    let v = g.view;
    for i in 1..20 {
        for j in 1..20 {
            let vx = v.xmin + (v.xmax - v.xmin) * i as f64 / 20.0;
            let vy = v.ymin + (v.ymax - v.ymin) * j as f64 / 20.0;
            let (x, y) = dev(&project, vx, vy);
            if let Some(ElementId::AxisBar { .. }) = res.info.hit_test(x, y, 2.0) {
                panic!("axis hit in plot interior at view ({vx:.3}, {vy:.3})");
            }
        }
    }
}

/// Self-consistency across the corpus: every element that drew something has
/// finite selection bounds, and every visible graph and non-hidden set is
/// represented in the recording.
#[test]
fn recorded_elements_are_consistent() {
    let fonts = FontSet::load();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "agr") {
            continue;
        }
        let project = oxygrace::load(&path).unwrap();
        let info = oxygrace::render_pixmap(&project, &fonts).info;
        for id in info.elements() {
            let b = info.bounds(id).unwrap_or_else(|| panic!("{path:?}: no bounds for {id:?}"));
            assert!(
                b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite(),
                "{path:?}: non-finite bounds for {id:?}: {b:?}"
            );
            assert!(b.x0 <= b.x1 && b.y0 <= b.y1, "{path:?}: inverted bounds for {id:?}");
        }
        for (g, graph) in project.graphs.iter().enumerate() {
            if graph.hidden {
                continue;
            }
            assert!(
                info.bounds(ElementId::Graph(g)).is_some(),
                "{path:?}: visible graph {g} recorded nothing"
            );
            for (s, set) in graph.sets.iter().enumerate() {
                if !set.hidden && !set.data.is_empty() {
                    assert!(
                        info.bounds(ElementId::Set { graph: g, set: s }).is_some(),
                        "{path:?}: visible set G{g}.S{s} recorded nothing"
                    );
                }
            }
        }
    }
}
