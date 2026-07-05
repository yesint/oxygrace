//! The native `.oxgr` project format.
//!
//! A single RON document: structured settings mirrored from the model by
//! serde, with datasets inlined as raw-string blocks of whitespace-separated
//! rows — the `.agr` data experience inside a standard grammar. Raw strings
//! escalate their delimiter (`r#"…"#`, `r##"…"##`) automatically, so any
//! data content round-trips.
//!
//! Unlike the tolerant `.agr` reader, `.oxgr` is *our* format: malformed
//! documents fail loudly with a position. Unknown **fields** however are
//! skipped (serde's default), so files from newer minor revisions still
//! load — adding a feature is adding a `#[serde(default)]` field in
//! [`v1`], nothing else.

use crate::model::Project;

pub mod v1;

/// Errors from reading an `.oxgr` document.
#[derive(Debug)]
pub enum OxgrError {
    /// RON syntax / schema error, with position info.
    Syntax(ron::error::SpannedError),
    /// The document declares a format revision this build cannot read.
    Version(u32),
    /// An unparsable color entry (`rgb` must be `#rrggbb`).
    Color { id: i32, rgb: String },
}

impl std::fmt::Display for OxgrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OxgrError::Syntax(e) => write!(f, "oxgr syntax error: {e}"),
            OxgrError::Version(v) => {
                write!(f, "oxgr format {v} is newer than this build supports (max 1)")
            }
            OxgrError::Color { id, rgb } => {
                write!(f, "oxgr color {id}: bad rgb {rgb:?} (expected \"#rrggbb\")")
            }
        }
    }
}

impl std::error::Error for OxgrError {}

/// RON options for `.oxgr`: `implicit_some` lets optional sections be
/// written (and hand-authored) as plain values instead of `Some(...)`.
fn ron_options() -> ron::Options {
    ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

/// Parse `.oxgr` text into a [`Project`].
pub fn load_oxgr_str(content: &str) -> Result<Project, OxgrError> {
    let doc: v1::Document = ron_options().from_str(content).map_err(OxgrError::Syntax)?;
    if doc.format != 1 {
        return Err(OxgrError::Version(doc.format));
    }
    doc.into_project()
}

/// Serialize a [`Project`] to `.oxgr` text.
pub fn save_oxgr_str(project: &Project) -> String {
    let doc = v1::Document::from_project(project);
    // `escape_strings(false)` makes the serializer emit multiline strings
    // (titles, datasets) as raw strings instead of `\n`-escaped ones —
    // that is what keeps the inline data readable. The implicit_some
    // extension is declared in the emitted header, so stock RON readers
    // parse the document too.
    let cfg = ron::ser::PrettyConfig::new()
        .escape_strings(false)
        // Style leaf structs (line/symbol/fill, axis slots) print inline;
        // documents stay indented above that.
        .depth_limit(5)
        .extensions(ron::extensions::Extensions::IMPLICIT_SOME);
    let body = ron_options()
        .to_string_pretty(&doc, cfg)
        .expect("oxgr schema structs always serialize");
    format!("// oxygrace project — .oxgr format 1\n{body}\n")
}
