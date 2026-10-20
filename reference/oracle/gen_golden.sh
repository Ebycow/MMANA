#!/bin/sh
# Regenerates testdata/golden from testdata/maa with the reference oracle.
# Each case: <stem>.json (file as-is) plus ground/frequency variants for a
# subset so every engine path (free space, perfect, real ground) is covered.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
ORACLE="$ROOT/reference/oracle/build/oracle"
OUT="$ROOT/testdata/golden"
mkdir -p "$OUT"
rm -f "$OUT"/*.json
for f in "$ROOT"/testdata/maa/*.MAA; do
	stem=$(basename "$f" .MAA)
	"$ORACLE" "$f" > "$OUT/$stem.json"
done
for stem in 3EL20 DP20 GP40 LOOP20 HB9CV VERT20 DP160LD MAGLOOP 4X12 G5RV; do
	f="$ROOT/testdata/maa/$stem.MAA"
	for g in 0 1 2; do
		"$ORACLE" "$f" --ground $g > "$OUT/$stem@ground$g.json"
	done
	"$ORACLE" "$f" --ground 2 --height 7.5 > "$OUT/$stem@h7.5.json"
done
for stem in 3EL20 DP20 LPDA 5EL20; do
	for fq in 13.9 14.35 21.2; do
		"$ORACLE" "$ROOT/testdata/maa/$stem.MAA" --freq $fq > "$OUT/$stem@f$fq.json"
	done
done
for stem in 3EL20 DP20 GP40 MAGLOOP; do
	f="$ROOT/testdata/maa/$stem.MAA"
	"$ORACLE" "$f" --near E > "$OUT/$stem@nearE.json"
	"$ORACLE" "$f" --near H > "$OUT/$stem@nearH.json"
	"$ORACLE" "$f" --far 1 > "$OUT/$stem@far.json"
	"$ORACLE" "$f" --ground 0 --far 1 > "$OUT/$stem@ground0far.json"
done
ls "$OUT" | wc -l
