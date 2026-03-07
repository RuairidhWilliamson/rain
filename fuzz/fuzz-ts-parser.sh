#!/bin/bash

set -euo pipefail

fuzz=$(dirname "$0")

"$fuzz/cargo-fuzz.sh" run fuzz_ts_parser -j 4 "$fuzz/corpus/fuzz_ts_parser" "$fuzz/../core/tests/scripts" "$fuzz/../core/tests/errors"
