# HyperNavigator Desktop - Product Requirements Document

**Status:** Proposed, implementation-ready MVP
**Revision:** 0.1.0
**Date:** 2026-10-14
**Initial target:** macOS Apple Silicon (arm64)
**Product:** HyperNavigator (the successor name to the earlier DotWeb concept)
**Owners:** HyperNavigator engineering

## 1. Executive summary

HyperNavigator is a browser-like desktop application for discovering, loading, displaying, and eventually granting controlled local capabilities to remotely distributed applications authored using real Kotlin Compose syntax.

The MVP must prove this precise path:

1. An application developer writes a Kotlin `@Composable` UI.
2. The developer compiles it to **Kotlin/Wasm** with Compose Multiplatform for Web and publishes a static distribution over HTTP.
3. HyperNavigator fetches a small manifest, validates the local development URL and entrypoint, and navigates a dedicated application WebView to the distribution.
4. The Compose UI renders **inside the HyperNavigator window**, on a canvas through Compose's web renderer, and its state, buttons, and recomposition work.
5. HyperNavigator can optionally prompt the user for a local folder and send a narrowly scoped, read-only directory listing to the active application via a Host-controlled bridge.

**The MVP does not run Android APKs, Android Activities, or downloaded JVM JARs.** It must not convert the remote composable source into mock HTML or a hand-written JSON component tree. The real Kotlin/Compose compilation and runtime are mandatory.

**Security designation:** Developer-only, trusted-localhost proof of concept. It is *not* yet a safe runtime for arbitrary untrusted internet applications. The application must make this limitation unmistakable in its UI and README.

## 2. Product thesis

Conventional web browsers distribute interfaces through HTML/CSS/JavaScript. HyperNavigator experiments with a different application authoring surface: Kotlin and Compose Multiplatform. An application can live on a remote static HTTP server and run locally within a desktop window, with future permission-based access to files, tools, and services.

The product's ultimate value proposition is locally executing agentic software with explicit, auditable capabilities. The v0.1 product proves the distribution and rendering architecture before making broader security claims.

## 3. Goals and non-goals

### 3.1 MVP goals (P0)

- Launch a usable native macOS HyperNavigator window with browser-like chrome: address bar, Go, Reload, Back, Forward, loading/error state.
- Open `http://127.0.0.1:8765/hypernavigator.json` and resolve a same-origin Compose/Wasm application entrypoint.
- Render the resulting `@Composable` interface in a dedicated **child WebView in the same native window**, beneath the trusted chrome.
- Demonstrate a working `remember { mutableIntStateOf(0) }` counter and click-driven recomposition.
- Refresh and rebuild the remote application without recompiling the HyperNavigator desktop shell.
- Gracefully handle invalid manifests, broken URLs, unsupported bundle types, and unavailable servers.
- On user request, select one local folder using a native picker, list up to 100 immediate child entries in Rust, and share **only sanitized names and types** with the currently active sample app after an explicit confirmation.
- Enforce a narrow development-only URL policy and expose no privileged Tauri commands to the application WebView.
- Include reproducible build/run instructions, tests, and a demonstrable working example.

### 3.2 Stretch goals (P1; do not block P0)

- Multi-tab support, full navigation history, per-app title and favicon, local HTTP(S) asset cache.
- An extensible, request/response, permission-gated broker for file reads/writes.
- Signed manifests, bundle digest verification, publisher identity, version pinning, and rollback.
- A managed permission dashboard with revoke/restart flows.
- Robust resource budgets, additional operating systems, installer and code-signing pipeline.

### 3.3 Explicit non-goals for v0.1

- Android APK loading, Android emulation, Android Activities, and Android-specific Compose APIs.
- Downloading JVM bytecode/JARs and injecting it into the trusted Host JVM.
- Running downloaded Kotlin source through a compiler upon every navigation.
- Supporting arbitrary public internet URLs or claiming arbitrary downloaded apps are safe.
- Unrestricted native filesystem, terminal, OS process, SSH, or secrets access.
- General HTML browsing, Chromium compatibility, full browser extension support, accounts, app stores, or AI agent orchestration.
- Replacing Skia, Compose Runtime, Kotlin compiler, or the operating system's WebView.

## 4. Users and jobs to be done

**Primary user:** Kotlin / Compose developer prototyping remotely distributed applications. Needs a low-friction way to publish one build to a static server and open it in HyperNavigator.

**Secondary user:** Developer evaluating local-first agentic interfaces. Needs visible permission requests and evidence that user-granted resources, not ambient OS privileges, govern application access.

**Future user (not MVP):** General desktop user opening third-party agentic apps safely.

## 5. Architecture decision (ADR-001)

### Selected: Rust + Tauri 2 shell, Kotlin/Wasm + Compose Multiplatform renderer

- Tauri's Rust Core owns URL validation, application session lifecycle, native file picker, permission state, and other OS operations.
- A trusted local webview renders the browser chrome using minimal TypeScript + Vite (no React required).
- An **independent guest child WebView** renders the remote Kotlin/Wasm app, using the real Compose Multiplatform Web backend and `ComposeViewport`.
- Rust creates both child WebViews in a single native window using Tauri 2's **unstable** multi-webview API (`Window::add_child`, `WebviewBuilder`); it maintains viewport bounds when the window resizes.
- The guest is not allowed to call Tauri native commands. Only a fixed, validated, Host-initiated channel transfers optional folder-listing data into the app.

### Why not Compose Desktop/JVM inside Tauri?

Compose Desktop/JVM uses a Skia-backed JVM window/panel, while Tauri's viewport is an OS-provided WebView. Tauri does not directly mount a `ComposePanel` or JVM scene inside a WebView. Doing so cross-process would require custom native window embedding or offscreen frame streaming and input forwarding. That is out of scope for the minimum proof of concept.

The Kotlin/Wasm target **does** use real Compose syntax and rendering, but it uses the **Compose for Web** canvas backend rather than the Desktop/JVM renderer. This tradeoff is intentional. If the non-negotiable requirement becomes *specifically* the Desktop/JVM renderer or unrestricted JVM libraries, the correct alternative is a Compose Desktop/Kotlin shell plus a Rust security sidecar, not Tauri WebView.

### Important technology constraints

- Compose for Web / Kotlin-Wasm is Beta and requires modern WebKit with WasmGC and exception-handling support. Verify support on the target macOS runtime; fail with a clear compatibility error.
- Tauri's multi-webview implementation requires the `unstable` Cargo feature and may change. Pin the known working release and record any adaptations.
- The app is delivered as a **compiled static web distribution** (Wasm, JS glue, HTML, fonts/assets). Calling it a 'bundle' does not mean a single `.jar`, `.apk`, or standalone `.wasm` file will work by itself.
- Java NIO is not available to Kotlin/Wasm web apps. Filesystem access must be Host-mediated, not direct JVM file access.

## 6. MVP technology stack

| Concern | Choice | Notes |
|---|---|---|
| Host | Rust stable + Tauri 2 | Candidate pinned core: Tauri 2.12.3; feature `unstable` for child WebViews |
| Host browser chrome | TypeScript + Vite, vanilla DOM | Minimize frontend dependencies |
| Guest runtime | OS WebView: WKWebView on macOS | Guest renders within same native window, not a popup |
| Guest application source | Kotlin Multiplatform | Actual `@Composable` functions |
| Guest compilation | `wasmJs` via Gradle | Kotlin 2.4.20 candidate; use matching Compose Compiler Gradle plugin |
| Guest UI | Compose Multiplatform 1.12.1 candidate | `ComposeViewport`, Canvas/Skia-backed Compose web renderer |
| JDK for build | JDK 21 | Build-time only; end users do not need a JVM for a packaged Wasm app |
| Guest hosting | Local Python HTTP server | `127.0.0.1:8765` for demo only |
| Native dialog | Rust `rfd` or compatible native folder dialog | Return no raw paths to guest |
| Manifest | JSON / Serde | Versioned `hypernavigator.json` |
| Navigation | Rust-owned | Validate origins, schemes, redirects and same-origin entry |
| Testing | Rust unit tests, TS tests, Kotlin build, manual GUI smoke test | Security checks included |

Pin exact compatible versions after verification from official release tables. Commit the resulting `Cargo.lock`, npm lockfile, and Gradle wrapper/configuration. Do not assume a Kotlin plugin version is interchangeable with a Compose plugin version.

## 7. Runtime topology

```text
                 HTTP (localhost-only for MVP)
         http://127.0.0.1:8765/counter/
          hypernavigator.json + index.html
                *.mjs + *.wasm + assets
                          |
                          v
        +--------------------------------------+
        | HyperNavigator native desktop window |
        |                                      |
        | +----------------------------------+ |
        | | Trusted browser chrome WebView   | |
        | | address | Go | Back | Reload     | |
        | +----------------------------------+ |
        | | Independent guest child WebView  | |
        | | Kotlin/Wasm + ComposeViewport    | |
        | | Canvas: counter, file list       | |
        | +----------------------------------+ |
        +-------------------|------------------+
                            |
                       Rust Tauri Core
                 URL policy + native dialogs
                 ephemeral permission grants
                            |
                 host-authorized file listing
```

The guest must never have the same Tauri command capabilities as the chrome. Do not use a privileged same-webview `<iframe>` shortcut. Creating a child WebView is an explicit milestone, not an implementation detail to silently drop.

## 8. Developer application format

### 8.1 Distribution structure

```text
counter/
  hypernavigator.json
  index.html
  counter.mjs              # build-generated name may differ
  counter.wasm             # build-generated name may differ
  assets/                  # generated CSS, JS, fonts and resources
```

Do not depend on those example generated filenames; the Compose/Kotlin build tool determines actual names and imports.

### 8.2 v1 manifest example

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

Required validation:

- Maximum manifest response body: 64 KiB; content must be valid UTF-8 JSON.
- `schemaVersion` must equal 1; `runtime` must equal `compose-wasm-web`.
- Nonempty bounded `id`, `name`, and semver-like `version` strings.
- The `entry` must be a relative, normalized, same-origin path with no userinfo, query-based scheme smuggling, traversal, absolute URL, or forbidden encoding.
- Accept only `http://127.0.0.1:8765/` as the POC origin; reject other hosts, ports, schemes, cross-origin redirects, and external navigations.
- A remote page cannot change the privileged browser chrome URL or cause the Host to open arbitrary OS resources.
- Do not call an unsigned manifest trusted. This is explicitly a local dev format pending production signatures.

### 8.3 Sample Compose program

The sample must be a real Kotlin/Wasm application. A minimal UI:

```kotlin
@Composable
fun CounterApp() {
    var count by remember { mutableIntStateOf(0) }
    Column {
        Text("Hello from HyperNavigator")
        Text("Clicked $count times")
        Button(onClick = { count++ }) {
            Text("Increment")
        }
    }
}
```

The WASM browser entrypoint must attach the composition to `index.html` with `ComposeViewport` following the current official Compose documentation. If the Compose version requires `@OptIn(ExperimentalComposeUiApi::class)`, use it.

### 8.4 Local dev workflow

1. Build sample with `./gradlew :examples:counter:wasmJsBrowserDistribution` (adapt module path if needed).
2. Publish the generated production distribution plus `hypernavigator.json` under `counter/`.
3. Serve on `127.0.0.1:8765` using the included script and Python's static server or equivalent.
4. Launch HyperNavigator using `npm run tauri:dev` or the documented command.
5. Navigate to `http://127.0.0.1:8765/counter/hypernavigator.json`.
6. Click the Compose button and verify recomposition; rebuild and reload the app without rebuilding the Host.

Provide convenience scripts that automate these steps and document prerequisites (Rust, Xcode CLT, Node, JDK, Gradle wrapper).

## 9. UX requirements

### 9.1 Window and navigation

- App name shown as HyperNavigator, not DotWeb.
- Minimal polished dark/light capable chrome with editable address bar, Go, Back, Forward, Reload.
- Distinct loading, ready, and error states; meaningful failure text if server, manifest, or Wasm feature checks fail.
- A persistent banner or clear status indicator: `Developer preview - localhost bundles only`.
- Guest WebView fills all remaining window space beneath chrome and resizes correctly with window / DPI changes.
- The counter app cannot manipulate chrome controls or render over them.
- URL must represent the manifest URL; guest assets are loaded from its resolved `entry` URL.

### 9.2 Permission UX - read-only demo

- A trusted browser chrome action `Share folder names` opens a native folder picker.
- The Host shows the chosen directory name and the precise scope: immediate entries' filenames and types only, not file content or absolute paths.
- After the user's explicit confirmation, Rust enumerates up to 100 immediate entries and sends the filtered result to the current guest via a structured, versioned message/event.
- Grant lasts for current application session; navigation clears it. No unprompted file access and no persistent OS permissions in P0.
- Guest Compose sample receives and displays the file names using Kotlin/Wasm JS interop and Compose state; recompose visibly.
- Reject symlinks or do not dereference them; never recursively descend. Avoid sending raw host paths.

## 10. Functional requirements and acceptance tests

| ID | Priority | Requirement | Acceptance criterion |
|---|---|---|---|
| HN-001 | P0 | Native Tauri shell | One macOS window launches with trusted chrome and guest viewport |
| HN-002 | P0 | Native child WebView | Guest and chrome are separate child WebViews in the same window |
| HN-003 | P0 | URL/manifest | Valid local manifest resolves; invalid or remote manifests reject visibly |
| HN-004 | P0 | Real Compose rendering | `@Composable` Kotlin/Wasm counter appears in guest canvas |
| HN-005 | P0 | Input and state | Each click increments counter using `remember` state and recomposition |
| HN-006 | P0 | Remote update | Rebuild and browser reload shows new text without Host rebuild |
| HN-007 | P0 | Navigation | Go, Back, Forward, Reload work for allowed local app navigation; unsupported histories fail clearly |
| HN-008 | P0 | Window resizing | Chrome remains fixed-height, guest fills remainder, no overlap |
| HN-009 | P0 | Permission-controlled demo | User picks folder and confirms; guest sees bounded names/types, no absolute paths |
| HN-010 | P0 | Restrict host APIs | Guest cannot access generic Tauri commands or shell/filesystem plugins |
| HN-011 | P0 | Error handling | Offline, invalid JSON, incompatible schema, missing entry, Wasm failure show errors |
| HN-012 | P0 | Build reproducibility | Repo builds with documented commands on macOS arm64 |
| HN-013 | P0 | Security labeling | UI/readme explicitly says trusted-localhost developer preview, not safe for untrusted code |
| HN-014 | P1 | Bundle authenticity | Signed manifests, digests, publisher keys, cache and rollback |
| HN-015 | P1 | General file API | Request/response broker with canonical path checks and per-operation permissions |

## 11. Security requirements and limitations

### 11.1 MVP hardening

- Trusted shell WebView and guest WebView have different identifiers/labels and explicitly separate capability definitions. Do not enable Tauri's global JavaScript bridge for guest content. Custom commands must be explicitly restricted via Tauri `AppManifest::commands(...)` plus chrome-scoped capabilities; Tauri documents that otherwise registered commands can default to all windows/webviews.
- Avoid `allow-all` plugin permissions, broad shell-execution APIs, generic FS plugins, wildcard remote-capability URLs, and privileged iframes.
- Restrict manifest fetching and guest navigation to `http://127.0.0.1:8765` using scheme, exact host, exact port, safe path, and redirect validation; include blocked-navigation tests.
- The Host cannot trust guest-provided filenames, origin fields, or JSON. Rust owns active-session identity and decides when and what it shares.
- Limit request size, timeouts, redirects, filesystem listing size, and filenames exposed. Validate JS evaluation payloads via JSON serialization (no string interpolation of unescaped user content).
- Limit permissions to read-only metadata for a single chosen folder; no file contents, write access, recursive walking, or process execution.
- A local HTTP bundle is deliberately unsigned and must only be used in the developer-controlled POC environment. Do not label it vetted or verified.

### 11.2 Honest boundaries

- Tauri capabilities are not a complete OS sandbox for untrusted native code, and WebView engines may have vulnerabilities.
- A Kotlin/Wasm application uses browser APIs and may still perform network requests according to WebView policies; P0 does **not** guarantee network egress isolation.
- Host-mediated access is not equivalent to giving the guest unrestricted direct filesystem access. That broader objective requires explicit future architecture.
- Future third-party bundle support requires verified signatures, sandbox/permission review, network controls, resource budgets, and security tests before enabling arbitrary URLs.

## 12. Proposed repository layout

```text
hypernavigator/
  PRD.md
  README.md
  package.json
  package-lock.json
  index.html
  vite.config.ts
  src/
    main.ts               # browser chrome
    styles.css
    api.ts
  src-tauri/
    Cargo.toml
    Cargo.lock
    tauri.conf.json
    capabilities/
      chrome.json
      guest.json           # no application privileges
    src/
      main.rs
      lib.rs
      manifest.rs
      navigator.rs
      permissions.rs
  settings.gradle.kts
  gradlew
  gradlew.bat
  gradle/wrapper/
  examples/
    counter/
      build.gradle.kts
      src/wasmJsMain/kotlin/
      src/wasmJsMain/resources/index.html
      hypernavigator.json
  scripts/
    build-example.sh
    serve-example.sh
  tests/
    ...
```

Use one top-level Gradle root with `:examples:counter` included in `settings.gradle.kts`. Keep only necessary modules; avoid a microservice, database, external API, or node backend.

## 13. Phased implementation

**Phase A - Real Compose proof:** Create independently built Kotlin/Wasm counter distribution and demonstrate rendering in a normal modern WebKit-compatible browser.

**Phase B - Tauri shell:** Build trusted browser chrome and a dedicated remote child WebView within one native window; wire manifest fetching, navigation, lifecycle, resize, and error state.

**Phase C - Host permissions demonstration:** Add read-only folder picker, explicit confirmation, bounded Rust directory listing, and host-to-guest event consumed by Compose state.

**Phase D - Quality gate:** Pin versions; add Rust parser/security tests, frontend validation tests, reproducible scripts, manual macOS GUI verification, and complete README.

Each phase ends with passing checks. If multi-WebView on the pinned Tauri version is broken on the current OS, report the blocker explicitly instead of silently replacing it with an iframe or unrelated renderer.

## 14. Definition of done

The MVP is complete only when all are true:

1. `npm install`, the Gradle Web distribution build, `cargo check`, and the Tauri dev launch pass on macOS arm64 with documented prerequisites.
2. HyperNavigator opens one native window containing a trusted address bar above an independently embedded guest child WebView.
3. The guest displays a real `@Composable` counter, with interactive state and correct resizing.
4. A modified remote Compose application appears after rebuild/reload without rebuilding HyperNavigator.
5. Invalid URL/manifests and forbidden external origins are rejected with user-visible errors.
6. A user-selected folder listing can be explicitly approved and displayed by the guest with no file contents or absolute paths disclosed.
7. Tauri capabilities, permission gating, and manifest validation are tested; guest has no privileged generic Tauri access.
8. README explains architecture, commands, constraints, exact tested versions, and the fact that this developer preview is **not** safe for arbitrary untrusted applications.
9. All main code, manifests, scripts, and tests are committed in the repository.

## 15. Future roadmap (not part of MVP)

- HN-0.2: signed HTTPS bundles, content-addressed cache, origin/publisher identity, version rollback and offline mode.
- HN-0.3: capability broker (`filesystem.read`, `filesystem.write`, `http.request`) with explicit grants and audits; revoke and isolation tests.
- HN-0.4: typed RPC SDK for agentic applications, controlled process/tool execution, per-app resource limits.
- HN-0.5: full tab management, bookmarks/history, developer tools, accessibility, telemetry opt-in.
- HN-1.0: third-party untrusted code support only after independent security hardening and OS-level threat-model validation.

## 16. Technical references (verified 2026-10-14)

- Tauri architecture and WebView process model: https://v2.tauri.app/concept/process-model/
- Tauri multi-WebView / unstable child WebView API: https://docs.rs/tauri/latest/tauri/window/struct.Window.html
- Tauri security capabilities and iframe caveat: https://v2.tauri.app/security/capabilities/
- Tauri iframe security advisory: https://github.com/tauri-apps/tauri/security/advisories/GHSA-57fm-592m-34r7
- Kotlin/Compose Wasm overview: https://kotlinlang.org/docs/wasm-overview.html
- Compose Web `ComposeViewport`: https://kotlinlang.org/docs/multiplatform/compose-css-styles.html
- Kotlin/Wasm distribution build: https://kotlinlang.org/docs/wasm-get-started.html
- Kotlin/Wasm JS interoperability: https://kotlinlang.org/docs/wasm-js-interop.html
- Compose compatibility: https://kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html
- Tauri release info: https://tauri.app/release/

