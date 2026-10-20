#!/bin/sh
# Regenerates testdata/elements/golden from testdata/maa with the element
# oracle: every sample through every script in testdata/elements/scripts.
# Each golden file is {"<script>": <oracle output>, ...}.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
ORACLE="$ROOT/reference/oracle/build/elements"
OUT="$ROOT/testdata/elements/golden"
mkdir -p "$OUT"
rm -f "$OUT"/*.json
for f in "$ROOT"/testdata/maa/*.MAA; do
	stem=$(basename "$f" .MAA)
	{
		sep="{"
		for s in "$ROOT"/testdata/elements/scripts/*.txt; do
			printf '%s"%s":' "$sep" "$(basename "$s" .txt)"
			"$ORACLE" "$f" "$s"
			sep=","
		done
		echo "}"
	} > "$OUT/$stem.json"
done
ls "$OUT" | wc -l
