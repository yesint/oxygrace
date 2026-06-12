//! Million-point stress benchmark (ignored; run with
//! `cargo test --release --test stress -- --ignored --nocapture`).
//!
//! Measures the costs an interactive editor pays per frame on a huge set:
//! full re-render, hover hit-testing, and selection-bounds queries.

use std::time::Instant;

use oxygrace::model::{Project, SymbolType};
use oxygrace::{ElementId, FontSet};

fn million_point_project(symbols: bool) -> Project {
    let mut p = Project::default();
    let g = p.graph_mut(0);
    let n = 1_000_000usize;
    // Deterministic pseudo-noise on a sine carrier.
    let mut seed = 0x2545F4914F6CDD1Du64;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    let xs: Vec<f64> = (0..n).map(|i| i as f64 / n as f64 * 100.0).collect();
    let ys: Vec<f64> = xs.iter().map(|x| (x * 0.5).sin() + 0.2 * noise()).collect();
    let set = g.set_mut(0, &Default::default());
    set.data.cols = vec![xs, ys];
    if symbols {
        set.symbol = SymbolType::Circle;
        set.symbol_size = 0.3;
        set.symskip = 0; // every point — worst case
    }
    oxygrace::import::autoscale_world(g);
    p
}

fn time<R>(label: &str, mut f: impl FnMut() -> R) -> R {
    let t = Instant::now();
    let r = f();
    println!("  {label}: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    r
}

#[test]
#[ignore]
fn stress_million_points() {
    let fonts = FontSet::load();

    for (name, symbols) in [("line only", false), ("line + 1M circle symbols", true)] {
        println!("[1M points, {name}]");
        let p = million_point_project(symbols);
        let res = time("render (cold glyph cache)", || oxygrace::render_pixmap(&p, &fonts));
        let res = {
            let mut last = res;
            for i in 0..3 {
                last = time(&format!("render warm #{i}"), || oxygrace::render_pixmap(&p, &fonts));
            }
            last
        };
        let v = p.graphs[0].view;
        let side = p.page_width.min(p.page_height) as f64;
        let (cx, cy) = (
            ((v.xmin + v.xmax) / 2.0 * side) as f32,
            (p.page_height as f64 - (v.ymin + v.ymax) / 2.0 * side) as f32,
        );
        time("hit_candidates x100 (hover cost)", || {
            for i in 0..100 {
                std::hint::black_box(res.info.hit_candidates(cx + (i % 7) as f32, cy, 6.0));
            }
        });
        time("bounds(set) x100 (overlay cost)", || {
            for _ in 0..100 {
                std::hint::black_box(res.info.bounds(ElementId::Set { graph: 0, set: 0 }));
            }
        });
        let shapes: usize = res.info.shapes_of(ElementId::Set { graph: 0, set: 0 }).count();
        println!("  recorded shapes for the set: {shapes}");
        // Dump for visual inspection of the decimation fidelity.
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
            .join(format!("target/stress_{}.png", if symbols { "symbols" } else { "line" }));
        std::fs::write(&out, res.pixmap.encode_png().unwrap()).unwrap();
        println!("  wrote {}", out.display());
    }
}
