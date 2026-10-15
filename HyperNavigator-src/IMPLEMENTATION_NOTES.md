# HyperNavigator 0.1 - implementation notes

Date: 2026-10-14. Machine: macOS 26.7, Apple Silicon (arm64), Xcode 27.0, Rust 1.98.1,
Node 22.22.2, JDK 17.0.14 (JBR), Python 3.14.7.

## Milestones

- [x] **A - real Compose proof.** Kotlin Multiplatform `wasmJs` + Compose Multiplatform 1.12.1 counter
  (`@Composable`, `remember { mutableIntStateOf(0) }`, `Column`/`Text`/`Button`, `ComposeViewport`),
  built with `wasmJsBrowserDistribution`, staged by `scripts/build-example.sh`, served by
  `scripts/serve-example.sh`, verified standalone in Chrome and later in WKWebView. (Now lives in
  `../WebAppExamples-src`; see "Project split" below.)
- [x] **B - Tauri shell.** One native window with two child WebViews (`chrome`, `guest-<n>`),
  manifest fetch/validation, manifest-level history, resize layout, errors, status and origin indicators.
- [x] **C - host permission demo.** Native folder picker (rfd), trusted confirmation, bounded Rust
  listing, one-way JSON event into the guest, Compose state update.
- [x] **D - quality gate.** Pinned versions and lockfiles, Rust/Node/smoke tests, ACL tests, runtime
  security probe, README, these notes.

## Key decisions

| Decision | Why |
|---|---|
| Browser code lives in `HyperNavigator-src/`; example apps in `WebAppExamples-src/` (separate Gradle root). | User decisions: nested layout, then a clean browser/examples split (see "Project split"). |
| **JDK 17** instead of the PRD's JDK 21. | User decision; only JDK 17 is installed. Kotlin 2.4.20 / Gradle 9.7.0 build Kotlin/Wasm fine on 17 (JDK is build-time only). |
| Gradle **9.7.0** wrapper (with distribution SHA-256). | Highest Gradle version the Kotlin 2.4.20 Gradle plugin is tested with (Kotlin compatibility table). |
| `org.jetbrains.compose.material:material:1.12.1` (Material 2), not Material 3. | Material 3's release line (latest stable 1.9.0) does not track CMP 1.12.x; Material 2 matches runtime/foundation/ui 1.12.1 exactly. Explicit coordinates are used instead of the deprecated `compose.*` accessors. |
| `tauri-build` pinned to `=2.7.2` (2.7.3 was released the same day). | Avoid a same-day release; Cargo.lock pins the rest. |
| Window created in Rust with `Window::builder` + two `add_child` WebViews; `app.windows` is empty. | Gives each WebView an explicit label so capabilities can target the `chrome` WebView, not the window that also holds the guest. |
| `TitleBarStyle::Transparent` on macOS. | Tauri's default "Visible" style turns on a full-size content view; child WebViews then sit under the title bar and WKWebView adds a ~32 pt top inset, which hid the chrome's status row (found during GUI testing). Transparent keeps the client area below the title bar; the title bar color follows the light/dark theme. |
| Chrome is a fixed **76 logical px**; guest fills the rest; bounds recomputed on `Resized`/`ScaleFactorChanged` from `inner_size().to_logical(scale_factor)`. | PRD fixed-height chrome; Retina-correct. `auto_resize` is not used because it scales proportionally. |
| **A brand-new incognito guest WebView per load** (Go/Back/Forward/Reload/error page), label `guest-<session>`; the previous one is closed. | `clear_all_browsing_data` on WKWebView is asynchronous with no completion wait (races a following navigation). A fresh non-persistent data store guarantees Reload picks up a rebuilt app, carries no storage between sessions, and makes "navigation clears the grant" structural. |
| **Manifest-level history.** | Simple, verifiable semantics; raw WebView history is not exposed. Failed fetches still create history entries (like a browser error page); URLs rejected by policy never do and leave the guest untouched. |
| **Redirects disabled** (`max_redirects(0)`); any 3xx is an error. | Simplest correct redirect policy; no "final URL" ambiguity. |
| **No HTTP connection pooling** (`max_idle_connections(0)`). | `python3 -m http.server` speaks HTTP/1.0 and closes after each response; reusing the GET connection for the entry `HEAD` intermittently failed with "Peer disconnected" (found in GUI testing, reproduced by `works_with_http10_servers_that_close_connections`, which fails without the fix). |
| Manifest must be `Content-Type: application/json`; unknown fields rejected; `permissions` must be empty. | Strict v1 contract; folder access is granted interactively, not declared. |
| Address bar accepts a folder URL ending in `/` and appends `hypernavigator.json`. | Convenience; the final URL is still canonical and validated. |
| Policy checks run on the raw input *and* the parsed URL. | The `url` crate normalizes `127.1`, `2130706433`, `0x7f.0.0.1` to `127.0.0.1` and decodes `%2e`; the raw-prefix check rejects these aliases. |
| Placeholder/error page is a bundled `placeholder.html`; the message travels in the URL fragment and is rendered with `textContent`. | Local, no eval; guest navigation policy allows only that exact page for placeholder sessions. |
| Share confirmation is rendered by the trusted chrome, which Rust temporarily expands to full window while **hiding the guest**. | A trusted, rich prompt (exact scope) that the guest cannot overlay or click-jack. |
| Host→guest delivery: `Webview::eval` of a fixed script whose only variable part is a **JSON string literal** (`serde_json::to_string` of the serialized payload, U+2028/2029 escaped), parsed in the guest with `JSON.parse`; the script also checks `location.origin`/`pathname` before dispatching `CustomEvent('hypernavigator:folder-listing')`. | Filenames never become JavaScript source. Test proves hostile names leave the surrounding code byte-identical. |
| Folder listing: `symlink_metadata` on the root (symlinked root rejected), `DirEntry::file_type` (never follows), no recursion, up to 10 000 entries scanned, sorted, truncated to 100; names sanitized (invalid UTF-8, control and bidi-override chars → U+FFFD, ≤ 255 chars). | PRD §9.2 / §11. Deterministic for folders with ≤ 10 000 entries. |
| Grants are bound to a request id **and** the guest session id; re-checked after the picker, after listing, and right before `eval`. | No stale grant reaches a different document. |
| `tauri::generate_context!()` is expanded once in a generic `context::<R>()` function. | The macro defines a `#[no_mangle]` Info.plist symbol; expanding it again for the mock-runtime ACL tests fails to link. |
| Kotlin/Wasm interop uses top-level `= js("...")` functions with `@OptIn(ExperimentalWasmJsInterop::class)`. | Documented interop; a Kotlin `(JsAny) -> Unit` lambda is passed as the event callback. |
| `wasm-check.js` in the example validates a minimal WasmGC module and shows a readable error on failure. | PRD "fail with a clear compatibility error". |

## Security constraints as implemented

* **Command scoping:** `build.rs` declares all 7 commands with `AppManifest::commands`, which
  autogenerates `allow-<command>` permissions *and* turns on ACL checks for every invoke. The only
  capability with permissions (`capabilities/chrome.json`) targets `"webviews": ["chrome"]` and local
  content; it grants those 7 commands plus `core:event:allow-listen`/`allow-unlisten`.
  `capabilities/guest.json` targets `guest-*` with **no** permissions and no remote URLs.
  `withGlobalTauri` is `false`.
* **Guest:** no capability, no `remote` URLs anywhere, incognito data store, `on_navigation` allows
  only `http://127.0.0.1:8765` (applies to all frames on macOS), `on_new_window` → Deny,
  `on_download` → false.
* **Chrome:** may only navigate to its own bundled pages (or the Vite dev server in debug builds);
  CSP `default-src 'self'; script-src 'self'; frame-src 'none'; …`.
* **No** shell, fs, http, dialog or opener plugins are installed; no command accepts a path.
* Honest boundaries (also in README): no network egress isolation, not an OS sandbox, Tauri's IPC
  bridge script is still injected into the guest (calls are rejected by the ACL), unsigned manifests,
  unstable Tauri API, Beta Kotlin/Wasm.

## Tests executed (all run on this machine)

| Check | Command | Result |
|---|---|---|
| Rust unit + ACL tests | `cd src-tauri && cargo test` | **46 passed, 0 failed** (policy 10, manifest/fetch 17, permissions 7, ACL 5, navigator 4, layout 3) |
| Clippy | `cargo clippy --all-targets` | clean (after boxing `ResolvedApp`) |
| Native build | `cargo check --all-targets`; `npx tauri build --no-bundle` (release) | pass; release binary launched and ran the app |
| TypeScript + Vite | `npm run build` (`tsc --noEmit` with TS 7.0.2, then `vite build`) | pass |
| Config/security audit + input shim | `npm test` | **12 passed** (8 audits + 4 shim tests) |
| Kotlin/Wasm build | `./gradlew :counter:wasmJsBrowserDistribution` via `WebAppExamples-src/scripts/build-example.sh` | BUILD SUCCESSFUL |
| Static serving smoke test | `WebAppExamples-src/scripts/smoke-test.sh` | **26 PASS, 0 failures** (manifest JSON/size/schema/field rules, entry, `webApp` container, `counter.js`, both `.wasm` files as `application/wasm` with Wasm magic, fixtures, real-Compose source checks) |
| Standalone browser | `index.html` opened in Chrome; 3 clicks; dispatched a test event | counter 3; 4 of 5 entries shown (unknown kind dropped) |
| GUI (WKWebView), driven with real mouse/keyboard events + `screencapture` | see acceptance table | see below |
| Runtime security probe | `HN_SECURITY_PROBE=1 npm run tauri:dev` | all 10 privileged calls `denied`, `file://` fetch failed, `window.open` returned null, off-origin navigation blocked |

## Second example: a ported Jetpack Compose screen (after the initial MVP)

`WebAppExamples-src/notes/` ports a typical Jetpack Compose screen (Material 3, ViewModel +
StateFlow, `collectAsStateWithLifecycle`, type-safe Navigation Compose, string and plural
resources) with its UI in `commonMain`. It needed **no HyperNavigator changes**. Porting changes are
listed in `WebAppExamples-src/README.md`. Library versions: Material 3 1.9.0 (the version CMP 1.12.1's
Gradle plugin maps to), Lifecycle 2.11.0, Navigation Compose 2.9.2, Compose Resources 1.12.1.
Verified: Gradle build (no warnings), 53/53 smoke checks, and in Chrome the list, navigation to the
editor, text input and save-and-return.

## Trackpad tap-to-click fix (after the initial MVP)

Reported by the user: tapping (tap-to-click) **Increment counter** on a MacBook trackpad did nothing,
while the page worked in another browser and synthetic CGEvent clicks worked in HyperNavigator.

* Evidence: a temporary on-page event logger showed taps arriving as trusted
  `pointerdown → mousedown → pointerup → mouseup → click`, but with **`buttons = 0` on `pointerdown`**
  (the whole tap lasts ~2 ms; no physical button is held).
* Cause: Compose Multiplatform for Web 1.12.1 passes `event.buttons` straight into the scene
  (`ComposeWindowInternal.web.kt`), and `ComposeSceneInputHandler` marks the pointer pressed only if
  `buttons.areAnyPressed`. A press with no buttons is therefore never a press, so `Button` never fires.
  Reproduced deterministically in Chrome by dispatching the same sequence with `buttons: 0` (ignored)
  vs `buttons: 1` (counted).
* Fix: `src-tauri/src/guest_input_shim.js`, injected into every guest WebView with
  `WebviewBuilder::initialization_script`. It wraps the `MouseEvent.prototype.buttons` getter so a
  `pointerdown`/`mousedown` reporting 0 returns the bit for `button`, as the UI Events spec requires.
  All other events are unchanged. Verified in Chrome (same taps: 2 → 5) and by
  `tests/guest-input-shim.test.mjs`.
* This is a Host-provided compatibility shim, so apps need no workaround. It is static, carries no data,
  and gives the guest no new capability.

## Project split (after the initial MVP)

The example apps moved out of the browser into `../WebAppExamples-src`, a standalone Kotlin/Gradle
project (Gradle wrapper, version catalog, `counter/` module, `fixtures/`, and the build/serve/smoke
scripts). HyperNavigator no longer contains Gradle, Kotlin or example files, and nothing in its build
reads them (`npm test` checks this); the two only meet over HTTP at `http://127.0.0.1:8765`.

* `:examples:counter` became `:counter`; output is `WebAppExamples-src/counter/build/dist/wasmJs/productionExecutable/`
  and the web server root is `WebServer-dist/` at the repository root (generated, git-ignored).
* The Rust test that `include_str!`-ed the example manifest was replaced by one that uses the
  built-in valid manifest. The example's source and manifest checks moved from `npm test` into
  `WebAppExamples-src/scripts/smoke-test.sh`, which now also applies HyperNavigator's manifest field rules.
* The `example:*` npm scripts were removed from the browser's `package.json`.

## Acceptance (PRD §10)

| ID | Result | Evidence |
|---|---|---|
| HN-001 Native Tauri shell | **PASS** | `npm run tauri:dev` and the release binary each open one window "HyperNavigator - Developer preview" with chrome and guest viewport (screenshots taken during testing). |
| HN-002 Native child WebView | **PASS** | `Window::add_child` creates `chrome` and `guest-<n>` in window `main` (`lib.rs`, `guest.rs`); ACL denial messages in the probe name `window "main", webview "guest-1"`; no iframe exists in the chrome (CSP `frame-src 'none'`). |
| HN-003 URL/manifest | **PASS** | Valid manifest loads the app. In the GUI, `https://example.com/…`, `file:///etc/passwd` and `http://localhost:8765/…` each showed a "Blocked scheme/host" error; 10 policy tests and 17 manifest/fetch tests cover the rest. |
| HN-004 Real Compose rendering | **PASS** | Kotlin `@Composable` `CounterApp` rendered by `ComposeViewport` on a canvas in the guest WKWebView (screenshot: "REMOTE KOTLIN COMPOSE APPLICATION / Hello from HyperNavigator!"). |
| HN-005 Input and state | **PASS** | Three real mouse clicks → "3 / Clicked 3 times"; release build: two clicks → "2". |
| HN-006 Remote update | **PASS** | Changed `BUILD_LABEL`, ran the example build script, pressed Reload: UI showed "Build 2 - rebuilt and reloaded, Host untouched"; Host PID 94630 before and after. |
| HN-007 Navigation | **PASS** | Went through 6 history entries; Back ×5 returned to the counter (Back disabled, Forward enabled); Forward showed the next entry; Reload re-fetched. Empty-history Back/Forward/Reload are disabled in the UI and return clear errors (`navigator.rs` tests). |
| HN-008 Window resizing | **PASS** | Resized to 640×520 and 1400×900: chrome stayed 76 pt, guest filled the rest, Compose re-laid out, no overlap. `layout.rs` tests. |
| HN-009 Permission-controlled demo | **PASS** | Picker cancel → nothing shared. Picked a test folder → trusted dialog (guest hidden) → "Share names" → Compose listed 6 sorted entries: symlinks shown as `symlink` (outside target not listed), `subdir` not descended, hostile name `evil");alert(1);(".txt` rendered as text, no paths or contents. Reload cleared the grant. |
| HN-010 Restrict host APIs | **PASS** | Rust ACL tests (guest from dev origin and from a local URL denied for all 7 commands + 6 plugin commands; chrome label with remote content denied) and the runtime probe inside WKWebView (all denied, `__TAURI__` absent). |
| HN-011 Error handling | **PASS (Wasm failure: partial)** | GUI showed specific errors for offline server, invalid JSON, schemaVersion 2, runtime `jvm-jar`, missing entry (404), escaping entry. The WasmGC-missing message in `wasm-check.js` is implemented but **not observed** (this WebKit supports WasmGC). |
| HN-012 Build reproducibility | **PASS on this machine** | All documented commands ran here from the lockfiles/wrapper. A clean-machine run was **not tested**. |
| HN-013 Security labeling | **PASS** | Persistent "Developer preview - localhost bundles only" badge, window title, placeholder warning, share dialog note, README banner. No "secure"/"verified" claims. |
| HN-014 / HN-015 (P1) | NOT TESTED / not implemented | Out of MVP scope. |

## Remaining gaps and follow-ups

* Wasm start-up failures are reported inside the guest page only; the Host does not detect them.
* No network egress restriction for the guest; no OS-level sandbox; no signatures (HN-014).
* The guest can dispatch `hypernavigator:folder-listing` to itself; that only affects its own UI.
  Data from the Host is untrusted input to the app as well.
* `tauri dev`'s file watcher occasionally missed a Rust change during testing; restarting
  `npm run tauri:dev` fixed it.
* Only macOS arm64 was tested. `platforms: ["macOS"]` in the capabilities must be extended before
  other platforms.
* Non-Latin/emoji filenames may need extra fonts in the Compose app (not tested).
