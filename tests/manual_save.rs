//! Manual helper (ignored by default): write re-saved corpus files under
//! `target/saved/` so they can be opened in QtGrace for compatibility checks:
//! `cargo test --test manual_save -- --ignored`

#[test]
#[ignore]
fn dump_saved_corpus() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let out_dir = root.join("target/saved");
    std::fs::create_dir_all(&out_dir).unwrap();
    for entry in std::fs::read_dir(root.join("examples")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "agr") {
            continue;
        }
        let project = oxygrace::load(&path).unwrap();
        oxygrace::save(&project, out_dir.join(path.file_name().unwrap())).unwrap();
    }
}
