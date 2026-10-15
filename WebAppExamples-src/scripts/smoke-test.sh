#!/usr/bin/env bash
# Serves the staged <repo>/WebServer-dist/ on a temporary loopback port with the same `python3 -m http.server`
# used by serve-example.sh, then checks for every example that the manifest, its entry, the
# Kotlin/Wasm bootstrap and every referenced .wasm file are served with usable content types, that the
# manifest is one HyperNavigator will accept, and that the sources are real (portable) Compose code.
# Stops the server on exit.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVE_ROOT="$(cd "$ROOT/.." && pwd)/WebServer-dist"
for example in counter notes; do
    if [[ ! -f "$SERVE_ROOT/$example/hypernavigator.json" ]]; then
        echo "error: WebServer-dist/$example is not staged; run scripts/build-example.sh first" >&2
        exit 1
    fi
done

PORT="$(python3 -I -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$SERVE_ROOT" >/dev/null 2>&1 &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT

for _ in $(seq 1 50); do
    curl -s -o /dev/null "http://127.0.0.1:$PORT/" && break
    sleep 0.1
done

python3 -I - "http://127.0.0.1:$PORT" "$ROOT" "$SERVE_ROOT" <<'PY'
import json, pathlib, re, sys, urllib.request, urllib.error
from urllib.parse import urljoin

base = sys.argv[1]
root = pathlib.Path(sys.argv[2])
failures = []

def fetch(path, method="GET"):
    req = urllib.request.Request(base + path, method=method)
    try:
        with urllib.request.urlopen(req, timeout=5) as r:
            return r.status, r.headers.get("Content-Type", ""), r.read() if method == "GET" else b""
    except urllib.error.HTTPError as e:
        return e.code, e.headers.get("Content-Type", ""), b""

def check(cond, label):
    print(("PASS " if cond else "FAIL ") + label)
    if not cond:
        failures.append(label)

def check_app(app, script):
    """Served assets and manifest rules for one staged example under /<app>/."""
    status, ctype, body = fetch(f"/{app}/hypernavigator.json")
    check(status == 200 and ctype.split(";")[0] == "application/json", f"[{app}] manifest served as application/json ({status} {ctype})")
    check(len(body) <= 64 * 1024, f"[{app}] manifest is within 64 KiB")
    m = json.loads(body)
    check(m.get("schemaVersion") == 1 and m.get("runtime") == "compose-wasm-web", f"[{app}] manifest schemaVersion 1 / runtime compose-wasm-web")
    # The same field rules HyperNavigator enforces (src-tauri/src/manifest.rs).
    check(set(m) <= {"schemaVersion", "id", "name", "version", "runtime", "entry", "permissions"}, f"[{app}] manifest has no unknown fields")
    check(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", m.get("id", "")) is not None, f"[{app}] manifest id is valid")
    check(0 < len(m.get("name", "").strip()) and len(m["name"]) <= 64, f"[{app}] manifest name is 1-64 characters")
    check(re.fullmatch(r"\d+\.\d+\.\d+([-+][0-9A-Za-z.+-]+)?", m.get("version", "")) is not None, f"[{app}] manifest version is semver-like")
    check(m.get("permissions", []) == [], f"[{app}] manifest permissions are empty (v0.1)")
    entry = m.get("entry", "")
    check(re.fullmatch(r"[A-Za-z0-9._~-]+(/[A-Za-z0-9._~-]+)*\.html", entry) is not None and ".." not in entry.split("/"),
          f"[{app}] entry {entry!r} is a safe relative .html path")
    entry_url = urljoin(f"{base}/{app}/hypernavigator.json", entry)
    check(entry_url.startswith(f"{base}/{app}/"), f"[{app}] entry resolves inside the app folder")

    status, ctype, html = fetch(entry_url[len(base):])
    check(status == 200 and ctype.startswith("text/html"), f"[{app}] entry served as text/html ({status})")
    html = html.decode()
    check('id="webApp"' in html, f'[{app}] entry has the ComposeViewport container id="webApp"')
    check("<button" not in html.lower(), f"[{app}] entry has no HTML button (the UI comes from Compose)")
    scripts = re.findall(r'<script[^>]+src="([^"]+)"', html)
    check(script in scripts, f"[{app}] entry loads {script} ({scripts})")

    status, ctype, js = fetch(f"/{app}/{script}")
    check(status == 200 and "javascript" in ctype, f"[{app}] {script} served as JavaScript ({ctype})")
    wasm_files = sorted(set(re.findall(r"[0-9a-f]{16,}\.wasm", js.decode(errors="replace"))))
    check(len(wasm_files) >= 1, f"[{app}] {script} references compiled .wasm modules ({len(wasm_files)})")
    for name in wasm_files:
        status, ctype, data = fetch(f"/{app}/{name}")
        check(status == 200 and ctype == "application/wasm" and data[:4] == b"\0asm",
              f"[{app}] {name} served as application/wasm with a Wasm header ({len(data)} bytes)")

def kotlin_sources(*parts):
    return "".join(p.read_text(encoding="utf-8") for p in sorted(root.joinpath(*parts).rglob("*.kt")))

check_app("counter", "counter.js")
check_app("notes", "notes.js")

for fixture in ["invalid-json", "wrong-schema", "wrong-runtime", "missing-entry", "escaping-entry"]:
    status, _, _ = fetch(f"/fixtures/{fixture}/hypernavigator.json", "HEAD")
    check(status == 200, f"fixture {fixture} staged")

# counter: real Compose rendered by ComposeViewport, not an HTML imitation.
counter = kotlin_sources("counter", "src")
check(re.search(r"@Composable\s+fun CounterApp", counter) is not None and "remember { mutableIntStateOf(0) }" in counter and "Button(" in counter,
      "[counter] CounterApp is a @Composable using remember { mutableIntStateOf(0) } and Button")
check('ComposeViewport(viewportContainerId = "webApp")' in counter, "[counter] main() renders with ComposeViewport into #webApp")

# notes: Jetpack-Compose-style UI lives in commonMain and uses no Android-only APIs.
common = kotlin_sources("notes", "src", "commonMain")
check(re.search(r"^import android\.", common, re.M) is None, "[notes] commonMain has no android.* imports")
for api in ["androidx.compose.material3.Scaffold", "androidx.lifecycle.ViewModel", "androidx.navigation.compose.NavHost",
            "androidx.lifecycle.compose.collectAsStateWithLifecycle", "org.jetbrains.compose.resources.stringResource"]:
    check(f"import {api}" in common, f"[notes] commonMain uses {api.rsplit('.', 1)[1]}")
check('ComposeViewport(viewportContainerId = "webApp")' in kotlin_sources("notes", "src", "wasmJsMain"), "[notes] wasmJsMain main() uses ComposeViewport")
staged = pathlib.Path(sys.argv[3])
resources = sorted(p.relative_to(staged).as_posix() for p in (staged / "notes" / "composeResources").rglob("*") if p.is_file())
check(len(resources) >= 1, f"[notes] compose resources staged ({len(resources)})")
for path in resources:
    status, _, data = fetch("/" + path)
    check(status == 200 and len(data) > 0, f"[notes] {path} served")

check("dotweb" not in (counter + common).lower(), "no old DotWeb name in the examples")

print(f"\n{len(failures)} failure(s)")
sys.exit(1 if failures else 0)
PY
