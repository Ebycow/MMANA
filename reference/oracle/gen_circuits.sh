#!/bin/sh
# Regenerates testdata/circuits/golden.json from testdata/circuits/cases.txt
# with the calculator oracle (the case format is described in tools_main.cpp).
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
"$ROOT/reference/oracle/build/tools" "$ROOT/testdata/circuits/cases.txt" > "$ROOT/testdata/circuits/golden.json"
grep -c '"tool"' "$ROOT/testdata/circuits/golden.json"
