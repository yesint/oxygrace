#!/usr/bin/env bash
# Render reference PNG baselines with QtGrace6 (the behavioural reference).
#
# Usage:
#   scripts/baseline.sh            # render every examples/*.agr
#   scripts/baseline.sh axes bar   # render only the named examples
#
# Output goes to target/baseline/<name>.png (target/ is gitignored).
# QtGrace is a Qt app, so we run it headless with the offscreen platform.
# NOTE: the .agr file must come BEFORE the flags, or qtgrace ignores -hardcopy.
set -euo pipefail

QTGRACE="${QTGRACE:-/home/semen/install/QtGrace6/build/qtgrace}"
root="$(cd "$(dirname "$0")/.." && pwd)"
outdir="$root/target/baseline"
mkdir -p "$outdir"

if [[ ! -x "$QTGRACE" ]]; then
  echo "qtgrace not found at $QTGRACE (set QTGRACE=...)" >&2
  exit 1
fi

names=("$@")
if [[ ${#names[@]} -eq 0 ]]; then
  for f in "$root"/examples/*.agr; do names+=("$(basename "$f" .agr)"); done
fi

for name in "${names[@]}"; do
  src="$root/examples/$name.agr"
  [[ -f "$src" ]] || { echo "skip: $src not found" >&2; continue; }
  QT_QPA_PLATFORM=offscreen timeout 120 "$QTGRACE" \
    "$src" -hardcopy -hdevice PNG -printfile "$outdir/$name.png" -NoWizard \
    >/dev/null 2>&1 || echo "warn: qtgrace failed on $name" >&2
  [[ -f "$outdir/$name.png" ]] && echo "baseline: $name"
done
