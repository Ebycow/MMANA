#!/bin/sh
# Compares the calculation speed of the original C++ engine and the Rust port.
# Same work on both sides: segmentation, current solve and patterns for one
# frequency; median wall time in ms.
#
# usage: reference/oracle/bench.sh [STEM ...]   (default: samples of increasing size)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
mkdir -p "$HERE/build"
${CXX:-clang++} -std=c++14 -O2 -ffp-contract=off -w \
	-I"$HERE/shim" -I"$HERE/generated" -I"$HERE" \
	"$HERE/generated/mininec3.cpp" "$HERE/generated/comlib_extract.cpp" "$HERE/bench_main.cpp" \
	-o "$HERE/build/bench"
(cd "$ROOT" && cargo build --release -q -p mmana-core --example bench)
RUST="$ROOT/target/release/examples/bench"

[ $# -gt 0 ] || set -- VERT20 2EL20 GP40 TWIND COLLINEA 15EL23CM 3HENT 4X12 12EL23CM CR2
printf "%-10s %7s %10s %12s %12s\n" sample pulses "C++ [ms]" "Rust 1T" "Rust all"
for stem in "$@"; do
	out=$("$HERE/build/bench" "$ROOT/testdata/maa/$stem.MAA")
	pulses=${out% *} cpp=${out#* }
	one=$(RAYON_NUM_THREADS=1 "$RUST" --calc-only "$stem" | awk 'NR == 2 { print $3 }')
	all=$("$RUST" --calc-only "$stem" | awk 'NR == 2 { print $3 }')
	printf "%-10s %7s %10s %12s %12s\n" "$stem" "$pulses" "$cpp" "$one" "$all"
done
