#!/bin/sh
# Regenerates testdata/optimize/golden from testdata/optimize/cases with the
# optimisation oracle (the case format is described in optimize_main.cpp).
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
ORACLE="$ROOT/reference/oracle/build/optimize"
OUT="$ROOT/testdata/optimize/golden"
mkdir -p "$OUT"
rm -f "$OUT"/*.json
for c in "$ROOT"/testdata/optimize/cases/*.txt; do
	"$ORACLE" "$c" > "$OUT/$(basename "$c" .txt).json"
done
ls "$OUT" | wc -l
