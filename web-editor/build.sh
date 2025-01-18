#!/bin/bash
set -exuo pipefail
 
wasm-pack build --target web --no-typescript --release --no-pack --out-name rain -d public/rain
