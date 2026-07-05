//! Parsing of numeric data rows inside `.agr`/`.xvg` data blocks.

/// Parse one whitespace-separated numeric data row.
///
/// Returns `None` if the line contains no parseable leading numbers. Trailing
/// non-numeric tokens (e.g. string columns) are ignored for now.
pub fn parse_row(line: &str) -> Option<(Vec<f64>, Option<String>)> {
    let mut cols = Vec::new();
    let mut rest = line;
    loop {
        rest = rest.trim_start();
        let tok_end = rest
            .find(char::is_whitespace)
            .unwrap_or(rest.len());
        if tok_end == 0 {
            break;
        }
        match rest[..tok_end].parse::<f64>() {
            Ok(v) => cols.push(v),
            Err(_) => break, // stop at the first non-numeric token
        }
        rest = &rest[tok_end..];
    }
    if cols.is_empty() {
        return None;
    }
    // A trailing double-quoted token is the point's annotation string
    // (Grace data column `s`, used by avalue type 4). Embedded quotes are
    // written escaped (`\"`, like Grace's writer) — undo that here.
    let rest = rest.trim();
    let s = rest
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .map(|r| r.replace("\\\"", "\""));
    Some((cols, s))
}
