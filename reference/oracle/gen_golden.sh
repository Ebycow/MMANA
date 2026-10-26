#!/bin/sh
# Regenerates testdata/golden from testdata/maa (the samples MMANA ships)
# and testdata/models (models for engine paths no sample takes) with the
# reference oracle.
# Each case: <stem>.json (file as-is) plus ground/frequency variants for a
# subset so every engine path (free space, perfect, real ground) is covered.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
ORACLE="$ROOT/reference/oracle/build/oracle"
OUT="$ROOT/testdata/golden"
mkdir -p "$OUT"
rm -f "$OUT"/*.json
for f in "$ROOT"/testdata/maa/*.MAA "$ROOT"/testdata/models/*.MAA; do
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
# grounded slopers: the near field next to wires that meet their image
for k in E H; do
	"$ORACLE" "$ROOT/testdata/models/GNDSLOPE.MAA" --near $k > "$OUT/GNDSLOPE@near$k.json"
done
# real grounds of more than one medium, and a radial screen (program
# settings, not in the files; the golden carries them for the test)
"$ORACLE" "$ROOT/testdata/maa/GP40.MAA" --ground 2 --media '5,0.001,8,-;13,0.005,-,-0.5' --screen 30,8 > "$OUT/GP40@ground2screen.json"
"$ORACLE" "$ROOT/testdata/maa/GP40.MAA" --ground 2 --media '5,0.001,8,-;13,0.005,-,-0.5' --screen 30,8 --far 1 > "$OUT/GP40@ground2screenfar.json"
"$ORACLE" "$ROOT/testdata/maa/VERT20M.MAA" --ground 2 --media '5,0.001,15,-;80,4,-,-1' > "$OUT/VERT20M@ground2media.json"
"$ORACLE" "$ROOT/testdata/maa/VERT20M.MAA" --ground 2 --media '5,0.001,15,-;80,4,-,-1' --far 1 > "$OUT/VERT20M@ground2mediafar.json"
ls "$OUT" | wc -l
