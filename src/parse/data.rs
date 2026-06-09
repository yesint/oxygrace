//! Parsing of numeric data rows inside `.agr`/`.xvg` data blocks.

/// Parse one whitespace-separated numeric data row.
///
/// Returns `None` if the line contains no parseable leading numbers. Trailing
/// non-numeric tokens (e.g. string columns) are ignored for now.
pub fn parse_row(line: &str) -> Option<Vec<f64>> {
    let mut cols = Vec::new();
    for tok in line.split_whitespace() {
        match tok.parse::<f64>() {
            Ok(v) => cols.push(v),
            Err(_) => break, // stop at the first non-numeric token
        }
    }
    if cols.is_empty() {
        None
    } else {
        Some(cols)
    }
}
