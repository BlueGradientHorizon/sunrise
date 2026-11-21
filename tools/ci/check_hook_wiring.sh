#!/usr/bin/env bash
# Check engine bindings separately from vocabulary generation; build only the source checker.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.."
exec cargo run --quiet --locked -p sunrise-mod-api-gen --bin sunrise-hook-wiring -- "$PWD"
