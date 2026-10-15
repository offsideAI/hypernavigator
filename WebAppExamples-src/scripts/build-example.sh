#!/usr/bin/env bash
# Compiles every Kotlin/Wasm Compose example and stages each one, with its manifest, in
# <repo>/WebServer-dist/<example>/ (the web server root shared by all example apps).
set -euo pipefail

EXAMPLES=(counter notes)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVE_ROOT="$(cd "$ROOT/.." && pwd)/WebServer-dist"

cd "$ROOT"
TASKS=()
for example in "${EXAMPLES[@]}"; do
    TASKS+=(":$example:wasmJsBrowserDistribution")
done
./gradlew "${TASKS[@]}" --console=plain

for example in "${EXAMPLES[@]}"; do
    DIST="$ROOT/$example/build/dist/wasmJs/productionExecutable"
    STAGE="$SERVE_ROOT/$example"
    if [[ ! -f "$DIST/index.html" ]]; then
        echo "error: expected $DIST/index.html after the Gradle build" >&2
        exit 1
    fi
    rm -rf "$STAGE"
    mkdir -p "$STAGE"
    cp -R "$DIST/." "$STAGE/"
    cp "$ROOT/$example/hypernavigator.json" "$STAGE/hypernavigator.json"
done

# Invalid manifests for manual error-handling checks (see fixtures/README.md).
rm -rf "$SERVE_ROOT/fixtures"
mkdir -p "$SERVE_ROOT/fixtures"
for fixture in "$ROOT"/fixtures/*/; do
    cp -R "$fixture" "$SERVE_ROOT/fixtures/$(basename "$fixture")"
done

# Fail early if a staged manifest is not valid JSON or its entry is missing.
python3 -I - "$SERVE_ROOT" "${EXAMPLES[@]}" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
for example in sys.argv[2:]:
    stage = root / example
    manifest = json.loads((stage / "hypernavigator.json").read_text(encoding="utf-8"))
    if not (stage / manifest["entry"]).is_file():
        sys.exit(f"error: manifest entry {manifest['entry']!r} not found in {stage}")
    print(f"staged {manifest['name']} {manifest['version']} -> {stage}")
PY
