#!/usr/bin/env bash
# Serves <repo>/WebServer-dist/ (all staged example apps) on the only origin HyperNavigator v0.1 accepts:
# http://127.0.0.1:8765
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVE_ROOT="$(cd "$ROOT/.." && pwd)/WebServer-dist"

if [[ ! -f "$SERVE_ROOT/counter/hypernavigator.json" ]]; then
    echo "error: WebServer-dist/counter is not staged; run scripts/build-example.sh first" >&2
    exit 1
fi

echo "Serving $SERVE_ROOT at http://127.0.0.1:8765/ (Ctrl+C to stop)"
echo "Manifest: http://127.0.0.1:8765/counter/hypernavigator.json"
exec python3 -m http.server 8765 --bind 127.0.0.1 --directory "$SERVE_ROOT"
