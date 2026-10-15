# HyperNavigator (developer preview 0.1)

> **Trusted-localhost developer preview only.** HyperNavigator 0.1 loads unsigned, unverified code
> from `http://127.0.0.1:8765` and nothing else. It is **not** safe for untrusted applications and
> makes no claim of sandboxing remote code against all threats.

HyperNavigator is a browser-like macOS desktop app that opens remotely served applications written
as real Kotlin `@Composable` code. Applications are compiled ahead of time to **Kotlin/Wasm** with
**Compose Multiplatform for Web**, served as static files, described by a small `hypernavigator.json`
manifest, and rendered with the official `ComposeViewport` canvas renderer inside a WebView that is
embedded in the HyperNavigator window.

This folder is the browser only. The example apps it opens, including the Compose counter, are a
separate Kotlin/Gradle project in [`../WebAppExamples-src`](../WebAppExamples-src); the two connect
only over HTTP.

This uses the Compose **Web (Kotlin/Wasm, Skia-on-canvas) backend** running in the operating system's
WebView (WKWebView). It does **not** use the Compose Desktop/JVM renderer and does not run APKs, JARs or
any downloaded JVM code.

## Architecture

```text
 one native macOS window ("main")
 ├── child WebView "chrome"     trusted, bundled TypeScript/Vite UI (76 logical px tall)
 │     address bar · Go · Back · Forward · Reload · Share folder names · status · guest origin
 │     └── calls 7 Rust commands, granted only to the "chrome" WebView label (capabilities/chrome.json)
 └── child WebView "guest-<n>"  remote app from http://127.0.0.1:8765, no Tauri permissions
       Kotlin/Wasm + ComposeViewport → real Compose UI on a <canvas>
       └── receives data only via Host-initiated, one-way events (Webview::eval + CustomEvent)

 Rust core (src-tauri/src)
   policy.rs       exact-origin URL policy, entry resolution, guest navigation policy
   manifest.rs     bounded manifest fetch (5 s timeout, 64 KiB, no redirects) + schema v1 validation
   navigator.rs    manifest-level history (Back/Forward/Reload) and guest session ids
   guest.rs        guest WebView lifecycle: fresh incognito WebView per load, navigation/popup blocking
   layout.rs       fixed-height chrome, guest fills the rest; resize and Retina scale handling
   permissions.rs  read-only folder-name listing (≤100 entries, no symlink following, no paths)
   commands.rs     the chrome-only commands
   security_probe.rs  debug-only runtime check that the guest cannot reach Host commands
```

Both WebViews are separate `tauri::Webview`s created with Tauri's **unstable** multi-WebView API
(`Window::add_child`). The guest is never an `<iframe>` inside the chrome.

## Prerequisites

| Tool | Tested version | Notes |
|---|---|---|
| macOS | 26.7 (arm64, Apple Silicon) | WKWebView must support WasmGC (Safari 18.2+ engine) |
| Xcode / Command Line Tools | Xcode 27.0 | `xcode-select -p` must succeed |
| Rust | 1.98.1 stable | `rustup` toolchain |
| Node.js / npm | 22.22.2 / 10.9.7 | Vite 8 needs Node ≥ 20.19 or ≥ 22.12 |
| JDK | 17.0.14 (JetBrains Runtime) | Examples only (build-time). JDK 17+ works; the PRD's JDK 21 is not required |
| Gradle | 9.7.0 via the wrapper in `WebAppExamples-src` | examples only; no global Gradle needed |
| Python | 3.14 (any 3.x) | `python3 -m http.server` serves the examples |

Pinned project versions: Tauri **2.12.3** (`unstable` feature), tauri-build 2.7.2, wry 0.57.0,
`@tauri-apps/cli` 2.12.3, `@tauri-apps/api` 2.12.4, Vite 8.3.4, TypeScript 7.0.2. Lockfiles:
`src-tauri/Cargo.lock`, `package-lock.json`. The examples pin Kotlin **2.4.20** and Compose
Multiplatform **1.12.1** (see `../WebAppExamples-src/README.md`).

## Run it

Two terminals, both starting at the repository root:

```bash
# terminal A - build the Compose example, then serve it on http://127.0.0.1:8765 (Ctrl+C to stop)
cd WebAppExamples-src
scripts/build-example.sh       # compiles and stages WebServer-dist/counter/ (repo root)
scripts/serve-example.sh

# terminal B - launch HyperNavigator in development mode
cd HyperNavigator-src
npm install                    # first time only
npm run tauri:dev
```

In the HyperNavigator window the address bar already contains
`http://127.0.0.1:8765/counter/hypernavigator.json`. Press **Go**. The Compose counter appears in
the lower part of the window; click **Increment counter**.

### Rebuild the remote app without rebuilding HyperNavigator

1. Leave HyperNavigator and the server running.
2. Edit `WebAppExamples-src/counter/src/wasmJsMain/kotlin/dev/hypernavigator/examples/counter/CounterApp.kt`
   (for example change `BUILD_LABEL`).
3. Run `scripts/build-example.sh` in `WebAppExamples-src/`.
4. Press **Reload** in HyperNavigator. The new UI appears; the Host process is unchanged.

### Share folder names (permission demo)

1. With an app loaded, press **Share folder names** and pick a folder in the native macOS picker.
   Cancelling changes nothing.
2. HyperNavigator expands its trusted chrome over the hidden app and shows exactly what will be
   shared: the names and types (file, directory, symlink, other) of at most 100 items directly inside
   that folder. No paths, contents, sizes, dates, recursion or write access.
3. **Share names** sends one versioned JSON event to the current app session; the Compose app lists
   the entries. **Cancel** sends nothing. The grant ends when the app navigates, reloads, or another
   app is opened.

### Navigation semantics

* **History is manifest-level**: each Go adds the manifest URL to this window's history. Back, Forward
  and Reload re-fetch and re-validate the manifest and load the app into a **brand-new guest WebView**
  with an empty, non-persistent data store. Reload is therefore always a hard reload (fresh cache,
  no `localStorage` carried over).
* Navigations the app performs itself are not recorded. They are only allowed on
  `http://127.0.0.1:8765`; anything else (and every `window.open`) is blocked and reported in the
  status bar. Leaving the loaded entry document clears any folder grant.
* A URL ending in `/` is treated as `…/hypernavigator.json`.

## Manifest format (`hypernavigator.json`, schema v1)

```json
{
  "schemaVersion": 1,
  "id": "dev.hypernavigator.examples.counter",
  "name": "Compose Counter",
  "version": "0.1.0",
  "runtime": "compose-wasm-web",
  "entry": "index.html",
  "permissions": []
}
```

Rules enforced by the Host (`src-tauri/src/manifest.rs`, `policy.rs`):

* Manifest URL: exactly `http://127.0.0.1:8765/…/hypernavigator.json`; no other scheme, host, alias
  (`localhost`, `127.1`, `2130706433`, IPv6), port, userinfo, query, fragment, percent-encoding,
  backslash, or `.`/`..`/empty segments. Max 2048 characters.
* Response: HTTP 200, `Content-Type: application/json`, at most 64 KiB, UTF-8, fetched with a 5 s
  timeout, without a proxy and **without following redirects** (any 3xx is an error).
* `schemaVersion` = 1, `runtime` = `compose-wasm-web`, no unknown fields; `id` ≤ 128 chars of
  `[A-Za-z0-9._-]`; `name` 1–64 printable chars; `version` semver-like; `permissions` must be empty.
* `entry`: relative `.html` path of `[A-Za-z0-9._~-/]`, ≤ 256 chars, no `..`, resolved inside the
  manifest's folder on the same origin, and must answer `HEAD` with 200.

Invalid manifests for manual testing live in `../WebAppExamples-src/fixtures/` and are staged at
`http://127.0.0.1:8765/fixtures/<name>/hypernavigator.json`.

## Tests

```bash
(cd src-tauri && cargo test)        # 46 Rust tests: URL policy, manifest, fetch, history, layout,
                                    # folder listing, JS payload escaping, Tauri ACL (mock runtime)
npm test                            # 12 Node tests: config/security audits + the input shim
npm run typecheck && npm run build  # TypeScript check + Vite production build
npm run tauri:build                 # release binary, no bundle (src-tauri/target/release/hypernavigator)
```

Example-side checks (Gradle build, staging, served assets, manifest and Compose source) run from
`../WebAppExamples-src`:

```bash
scripts/build-example.sh && scripts/smoke-test.sh
```

Runtime security probe (debug builds only): `HN_SECURITY_PROBE=1 npm run tauri:dev`, then press Go.
The Host runs a script inside the guest that tries Host commands, `fs`/`shell`/`event`/`webview`/
`window` plugin commands, a `file://` fetch, `window.open` and an off-origin navigation, and prints
the results as `[hn-security-probe] …` in the terminal. Every call must say `denied`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| "Could not connect to http://127.0.0.1:8765" | Start `WebAppExamples-src/scripts/serve-example.sh`. Port 8765 must be free. |
| "Manifest entry "index.html" is not available (HTTP 404)" | Run `WebAppExamples-src/scripts/build-example.sh` first. |
| Guest shows "does not support WebAssembly GC" | Update macOS; Kotlin/Wasm needs a WebKit with WasmGC. |
| `Port 1420 is already in use` on `tauri:dev` | Another Vite/Tauri dev server is running; stop it. |
| Gradle fails with an unsupported JDK | Use JDK 17 or newer (`java -version`). |
| Old UI after rebuilding the example | Press **Reload** (not Go on a different URL); it creates a fresh WebView. |
| Blank guest, nothing in the status bar | Debug builds allow Web Inspector: right-click in the guest → Inspect Element. |

## Limitations (read before using)

* **Developer preview, trusted localhost only.** Manifests are unsigned; nothing is verified or vetted.
* Not an OS sandbox. Tauri capabilities restrict Tauri IPC only; a WebKit vulnerability or a bug in
  this Host could still expose the system.
* **No network egress control.** The guest can make its own HTTP requests (fetch, images, WebSocket)
  to any destination WebKit allows; only top-level/frame navigations are restricted.
* The guest page still contains Tauri's IPC bridge script (`window.__TAURI_INTERNALS__`); every call
  through it is rejected by the Tauri ACL (verified by tests and the runtime probe).
* Tauri's multi-WebView API is behind the `unstable` feature and may change; Compose Multiplatform for
  Web (Kotlin/Wasm) is Beta. Only macOS arm64 is tested.
* Wasm/runtime failures inside the app are shown in the guest view (by the app's own
  `wasm-check.js`), not in the Host status bar.
* HyperNavigator injects one small script into every app page: a fix for macOS trackpad
  tap-to-click, which WebKit reports with `buttons = 0` and Compose therefore ignored
  (`src-tauri/src/guest_input_shim.js`).
* Text rendering in the Compose canvas uses Compose's bundled fonts; some non-Latin or emoji file
  names may render as boxes.
