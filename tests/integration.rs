//! End-to-end tests: grammar parsing, the world→view transform, and a corpus
//! smoke test that every bundled example loads and renders without panicking.

use oxygrace::draw::axes::{format_value, major_ticks};
use oxygrace::model::{ScaleType, TickFormat};
use oxygrace::parse::grammar::{parse_line, Command};
use oxygrace::render::WorldTransform;

#[test]
fn parses_core_commands() {
    assert!(matches!(parse_line("version 50122"), Command::Version(50122)));
    assert!(matches!(
        parse_line("with g0"),
        Command::With { graph: 0, set: None }
    ));
    assert!(matches!(
        parse_line("WITH G2"),
        Command::With { graph: 2, set: None }
    ));
    assert!(matches!(
        parse_line("target G0.S1"),
        Command::Target { graph: 0, set: 1 }
    ));
    assert!(matches!(parse_line("type xy"), Command::TypeDecl(_)));
    assert!(matches!(parse_line("world 0, 0, 10, 20"), Command::World(_)));
    assert!(matches!(parse_line("    world xmin 110"), Command::World(_)));
    assert!(matches!(parse_line("xaxis tick major 0.5"), Command::Axis { .. }));
    assert!(matches!(parse_line("s0 line color 2"), Command::Set { .. }));
    assert!(matches!(parse_line("frame type 0"), Command::Frame(_)));
}

#[test]
fn unknown_commands_do_not_fail() {
    assert!(matches!(parse_line("totally bogus command here"), Command::Unknown));
    assert!(matches!(parse_line("g0 fixedpoint off"), Command::Unknown));
}

#[test]
fn major_tick_generation() {
    let ticks = major_ticks(0.0, 1.0, 0.5);
    assert_eq!(ticks, vec![0.0, 0.5, 1.0]);
    // Negative range with rounding.
    let ticks = major_ticks(-1.0, 1.0, 0.5);
    assert_eq!(ticks.len(), 5);
}

#[test]
fn tick_label_formatting() {
    assert_eq!(format_value(0.5, TickFormat::Decimal, 1), "0.5");
    assert_eq!(format_value(-0.0, TickFormat::Decimal, 1), "0.0");
    assert_eq!(format_value(2.0, TickFormat::General, 2), "2");
}

#[test]
fn view_to_world_round_trips() {
    use oxygrace::model::{Graph, ScaleType, World};
    use oxygrace::render::WorldTransform;
    for (xscale, yscale, xinvert, world) in [
        (ScaleType::Normal, ScaleType::Normal, false, World { xmin: -3.0, xmax: 7.0, ymin: 0.5, ymax: 9.5 }),
        (ScaleType::Logarithmic, ScaleType::Logarithmic, false, World { xmin: 0.1, xmax: 1000.0, ymin: 1.0, ymax: 100.0 }),
        (ScaleType::Normal, ScaleType::Logit, true, World { xmin: 0.0, xmax: 10.0, ymin: 0.05, ymax: 0.95 }),
    ] {
        let graph = Graph { xscale, yscale, xinvert, world, ..Default::default() };
        let wt = WorldTransform::new(&graph);
        for i in 1..10 {
            let f = i as f64 / 10.0;
            let wx = world.xmin + f * (world.xmax - world.xmin);
            let wy = world.ymin + f * (world.ymax - world.ymin);
            let (vx, vy) = wt.world_to_view(wx, wy);
            let (bx, by) = wt.view_to_world(vx, vy).expect("inverse in domain");
            assert!((bx - wx).abs() < 1e-9 * (1.0 + wx.abs()), "x round trip: {wx} vs {bx}");
            assert!((by - wy).abs() < 1e-9 * (1.0 + wy.abs()), "y round trip: {wy} vs {by}");
        }
    }
}

#[test]
fn world_to_view_linear() {
    let mut graph = oxygrace::model::Graph {
        world: oxygrace::model::World {
            xmin: 0.0,
            xmax: 10.0,
            ymin: 0.0,
            ymax: 100.0,
        },
        ..Default::default()
    };
    graph.view = oxygrace::model::View {
        xmin: 0.2,
        xmax: 0.8,
        ymin: 0.1,
        ymax: 0.9,
    };
    graph.xscale = ScaleType::Normal;
    graph.yscale = ScaleType::Normal;
    let wt = WorldTransform::new(&graph);
    // Midpoint of world maps to midpoint of view.
    let (vx, vy) = wt.world_to_view(5.0, 50.0);
    assert!((vx - 0.5).abs() < 1e-9);
    assert!((vy - 0.5).abs() < 1e-9);
    // Corners.
    let (vx, vy) = wt.world_to_view(0.0, 0.0);
    assert!((vx - 0.2).abs() < 1e-9 && (vy - 0.1).abs() < 1e-9);
}

#[test]
fn corpus_loads_and_renders() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("agr") {
            continue;
        }
        let project = oxygrace::load(&path)
            .unwrap_or_else(|e| panic!("loading {}: {e}", path.display()));
        // Rendering must not panic; PNG must be non-empty.
        let png = oxygrace::render_png(&project);
        assert!(!png.is_empty(), "empty PNG for {}", path.display());
        count += 1;
    }
    assert!(count > 0, "no example files found in {dir}");
}

/// Every corpus file must also render to a structurally sound SVG document.
#[test]
fn corpus_renders_svg() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("agr") {
            continue;
        }
        let project = oxygrace::load(&path)
            .unwrap_or_else(|e| panic!("loading {}: {e}", path.display()));
        let svg = oxygrace::render_svg(&project);
        assert!(
            svg.starts_with("<?xml") && svg.trim_end().ends_with("</svg>"),
            "malformed SVG for {}",
            path.display()
        );
        // Every element we emit is self-closing or a matched pair; a quick
        // structural check that something was actually drawn:
        assert!(
            svg.contains("<path "),
            "no drawing elements in SVG for {}",
            path.display()
        );
        count += 1;
    }
    assert!(count > 0, "no example files found in {dir}");
}
