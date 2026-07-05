//! Plain numeric data import — the xmgrace command-line data conventions
//! (`-xy file`, `-nxy file`, `-type TYPE file`).
//!
//! Data files are lines of whitespace-separated numbers; blank lines or `&`
//! separate datasets; `#`/`@` lines are ignored.

use crate::model::{Graph, Project, Set, SetType};
use crate::parse::data::parse_row;

/// Append sets parsed from plain numeric data to `project.graphs[graph]`.
///
/// With `nxy`, the first column is X and every further column becomes its
/// own XY set (xmgrace `-nxy`); otherwise each block feeds one set of
/// `set_type`, taking as many columns as the type uses. Returns the number
/// of sets added.
pub fn import_data_str(
    project: &mut Project,
    content: &str,
    set_type: SetType,
    nxy: bool,
    graph: usize,
) -> usize {
    // Split into &-or-blank-line separated blocks of numeric rows.
    type Block = (Vec<Vec<f64>>, Vec<Option<String>>);
    let mut blocks: Vec<Block> = Vec::new();
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut strs: Vec<Option<String>> = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('&') {
            if !rows.is_empty() {
                blocks.push((std::mem::take(&mut rows), std::mem::take(&mut strs)));
            }
            continue;
        }
        if line.starts_with(['#', '@']) {
            continue;
        }
        if let Some((row, s)) = parse_row(line) {
            rows.push(row);
            strs.push(s);
        }
    }
    if !rows.is_empty() {
        blocks.push((rows, strs));
    }

    let defaults = project.defaults;
    let g = project.graph_mut(graph);
    let before = g.sets.len();
    for (rows, strs) in blocks {
        let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if nxy {
            for c in 1..max_cols {
                let mut set = Set::with_defaults(&defaults);
                set.set_type = SetType::Xy;
                let (xs, ys): (Vec<f64>, Vec<f64>) = rows
                    .iter()
                    .filter(|r| r.len() > c)
                    .map(|r| (r[0], r[c]))
                    .unzip();
                std::sync::Arc::make_mut(&mut set.data).cols = vec![xs, ys];
                g.sets.push(set);
            }
        } else {
            let ncols = set_type.ncols().min(max_cols);
            let mut set = Set::with_defaults(&defaults);
            set.set_type = set_type;
            let data = std::sync::Arc::make_mut(&mut set.data);
            data.cols = (0..ncols)
                .map(|c| rows.iter().filter_map(|r| r.get(c).copied()).collect())
                .collect();
            data.strs = strs;
            g.sets.push(set);
        }
    }
    g.sets.len() - before
}

/// Set the graph's world window to its data extents (padding degenerate
/// ranges), like the reader does for projects without an explicit `@world`.
pub fn autoscale_world(graph: &mut Graph) {
    let mut xmin = f64::INFINITY;
    let mut xmax = f64::NEG_INFINITY;
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for set in &graph.sets {
        if let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) {
            for &x in xs {
                if x.is_finite() {
                    xmin = xmin.min(x);
                    xmax = xmax.max(x);
                }
            }
            for &y in ys {
                if y.is_finite() {
                    ymin = ymin.min(y);
                    ymax = ymax.max(y);
                }
            }
        }
    }
    if !xmin.is_finite() || !ymin.is_finite() {
        return;
    }
    if (xmax - xmin).abs() < f64::EPSILON {
        xmin -= 0.5;
        xmax += 0.5;
    }
    if (ymax - ymin).abs() < f64::EPSILON {
        ymin -= 0.5;
        ymax += 0.5;
    }
    graph.world = crate::model::World { xmin, xmax, ymin, ymax };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nxy_splits_columns_into_sets() {
        let mut p = Project::default();
        let n = import_data_str(&mut p, "1 10 100\n2 20 200\n3 30 300\n", SetType::Xy, true, 0);
        assert_eq!(n, 2);
        let sets = &p.graphs[0].sets;
        assert_eq!(sets[0].data.x().unwrap(), &[1.0, 2.0, 3.0]);
        assert_eq!(sets[0].data.y().unwrap(), &[10.0, 20.0, 30.0]);
        assert_eq!(sets[1].data.y().unwrap(), &[100.0, 200.0, 300.0]);
    }

    #[test]
    fn blocks_split_sets_and_autoscale() {
        let mut p = Project::default();
        let n = import_data_str(&mut p, "0 5\n1 6\n&\n2 -1\n3 4\n", SetType::Xy, false, 0);
        assert_eq!(n, 2);
        autoscale_world(&mut p.graphs[0]);
        let w = p.graphs[0].world;
        assert_eq!((w.xmin, w.xmax, w.ymin, w.ymax), (0.0, 3.0, -1.0, 6.0));
    }
}
