#!/bin/bash

set -euo pipefail

fuzz=$(dirname "$0")

"$fuzz/cargo-fuzz.sh" run fuzz_parser -j 4 "$fuzz/corpus/fuzz_parser" "$fuzz/../core/tests/scripts" "$fuzz/../core/tests/errors"
