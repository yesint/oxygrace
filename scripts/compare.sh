#!/usr/bin/env bash
# Render examples with oxygrace and build side-by-side comparison montages
# against the QtGrace6 baselines (ours on the left, reference on the right).
#
# Usage:
#   scripts/compare.sh             # all examples
#   scripts/compare.sh axes bar    # only the named examples
#
# Outputs (all under the gitignored target/):
#   target/out/<name>.png        oxygrace render
#   target/baseline/<name>.png   qtgrace render (created if missing)
#   target/compare/<name>.png    labelled side-by-side montage
#
# Visual review is the point: open target/compare/*.png and check that the
# expected elements (frame, axes, ticks/labels, data, symbols, fills, legend,
# annotations) are present and roughly placed. No pixel-level parity expected.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
outdir="$root/target/out"
basedir="$root/target/baseline"
cmpdir="$root/target/compare"
mkdir -p "$outdir" "$cmpdir"

cargo build --quiet --manifest-path "$root/Cargo.toml"
bin="$root/target/debug/oxygrace"

names=("$@")
if [[ ${#names[@]} -eq 0 ]]; then
  for f in "$root"/examples/*.agr; do names+=("$(basename "$f" .agr)"); done
fi

for name in "${names[@]}"; do
  src="$root/examples/$name.agr"
  [[ -f "$src" ]] || { echo "skip: $src not found" >&2; continue; }

  "$bin" "$src" -o "$outdir/$name.png" >/dev/null 2>&1 || { echo "warn: oxygrace failed on $name" >&2; continue; }

  # Generate the baseline on demand if it is missing.
  if [[ ! -f "$basedir/$name.png" ]]; then
    "$root/scripts/baseline.sh" "$name" || true
  fi

  if [[ -f "$basedir/$name.png" ]]; then
    convert "$outdir/$name.png" -resize 520x520 -bordercolor gray -border 2 /tmp/_o.png
    convert "$basedir/$name.png" -resize 520x520 -bordercolor gray -border 2 /tmp/_q.png
    montage /tmp/_o.png /tmp/_q.png -tile 2x1 -geometry +6+6 \
      -title "$name : oxygrace (left)  vs  qtgrace (right)" "$cmpdir/$name.png"
    echo "compare: $name"
  else
    echo "compare: $name (oxygrace only; no baseline)"
  fi
done
