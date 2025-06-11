#!/bin/bash

set -exuo pipefail
mkdir -p ~/.config/helix/runtime/queries/rain/
tree-sitter generate --abi 14
cp ./queries/highlights.scm ~/.config/helix/runtime/queries/rain/highlights.scm
hx --grammar build
