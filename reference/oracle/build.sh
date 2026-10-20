#!/bin/sh
# Builds the reference oracle from the verbatim MMANA sources in generated/.
# -ffp-contract=off keeps clang from fusing a*b+c into FMA, so the arithmetic
# matches Rust (which never contracts) operation for operation.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$HERE/build"
${CXX:-clang++} -std=c++14 -O2 -ffp-contract=off -w \
	-I"$HERE/shim" -I"$HERE/generated" \
	"$HERE/generated/mininec3.cpp" "$HERE/generated/comlib_extract.cpp" "$HERE/oracle_main.cpp" \
	-o "$HERE/build/oracle"
${CXX:-clang++} -std=c++14 -O2 -ffp-contract=off -w \
	-I"$HERE" -I"$HERE/shim" -I"$HERE/generated" \
	"$HERE/generated/mininec3.cpp" "$HERE/generated/comlib_extract.cpp" "$HERE/generated/cwgroup_extract.cpp" \
	"$HERE/elements_main.cpp" \
	-o "$HERE/build/elements"
echo "built $HERE/build/oracle and $HERE/build/elements"
