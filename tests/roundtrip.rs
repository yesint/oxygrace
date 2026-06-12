//! Writer round-trip tests across the example corpus.
//!
//! Two complementary checks:
//! 1. *Save stability*: `save(load(f))` re-loaded and re-saved is identical
//!    text — catches writer nondeterminism and reader/writer asymmetries.
//! 2. *Render equality*: the re-loaded project renders byte-identically —
//!    catches model fields the writer forgot (silent data loss).

use std::path::PathBuf;

fn corpus() -> Vec<PathBuf> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "agr"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    files
}

#[test]
fn save_is_stable() {
    for path in corpus() {
        let p1 = oxygrace::load(&path).unwrap();
        let s1 = oxygrace::save_str(&p1);
        let p2 = oxygrace::load_str(&s1);
        let s2 = oxygrace::save_str(&p2);
        if s1 != s2 {
            // Show the first differing line for a readable failure.
            let diff = s1
                .lines()
                .zip(s2.lines())
                .enumerate()
                .find(|(_, (a, b))| a != b);
            panic!(
                "save not stable for {path:?}: first diff {:?}",
                diff.map(|(i, (a, b))| format!("line {}: {a:?} vs {b:?}", i + 1))
            );
        }
    }
}

#[test]
fn saved_project_renders_identically() {
    for path in corpus() {
        let p1 = oxygrace::load(&path).unwrap();
        let p2 = oxygrace::load_str(&oxygrace::save_str(&p1));
        let png1 = oxygrace::render_png(&p1);
        let png2 = oxygrace::render_png(&p2);
        assert_eq!(png1, png2, "render differs after save/load: {path:?}");
    }
}
