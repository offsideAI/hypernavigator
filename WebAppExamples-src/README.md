# WebAppExamples

Example applications that run **inside** HyperNavigator. The browser lives in
[`../HyperNavigator-src`](../HyperNavigator-src); this folder is a separate Kotlin/Gradle project
and does not depend on it. The two only meet over HTTP: HyperNavigator fetches a
`hypernavigator.json` manifest from `http://127.0.0.1:8765` and loads the page it names.

> These are unsigned, localhost-only developer examples for the HyperNavigator developer preview.

## Contents

```text
WebAppExamples-src/
├── settings.gradle.kts          Gradle root; one module per example (include(":counter"))
├── build.gradle.kts             plugin declarations (versions in gradle/libs.versions.toml)
├── gradle/ gradlew gradlew.bat  Gradle 9.7.0 wrapper
├── notes/                       a typical Jetpack Compose screen, ported (see below)
│   ├── build.gradle.kts         + Material 3, Lifecycle/ViewModel, Navigation, Resources, serialization
│   └── src/
│       ├── commonMain/kotlin/…      NotesApp.kt, NotesScreens.kt, NotesViewModel.kt (Android-style code)
│       ├── commonMain/composeResources/values/strings.xml   strings and plurals (Android's res/values)
│       └── wasmJsMain/              Main.kt (ComposeViewport) + index.html, styles.css, wasm-check.js
├── counter/                     the Compose counter example
│   ├── build.gradle.kts         Kotlin Multiplatform wasmJs target + Compose Multiplatform
│   ├── hypernavigator.json      manifest (schema v1) HyperNavigator fetches first
│   └── src/wasmJsMain/
│       ├── kotlin/…/CounterApp.kt   @Composable UI (counter, folder-name list, BUILD_LABEL)
│       ├── kotlin/…/Main.kt         main(): ComposeViewport("webApp") { CounterApp() }
│       ├── kotlin/…/HostBridge.kt   receives the Host's one-way folder-listing event
│       └── resources/               index.html (<div id="webApp">), styles.css, wasm-check.js
├── fixtures/                    deliberately broken manifests for testing HyperNavigator's errors
├── scripts/
│   ├── build-example.sh         Gradle build + stage into ../WebServer-dist/
│   ├── serve-example.sh         python3 -m http.server 8765 --bind 127.0.0.1 --directory ../WebServer-dist
│   └── smoke-test.sh            serves ../WebServer-dist/ on a temporary port and checks it
```

The server root is **`WebServer-dist/` at the repository root** (generated, git-ignored). It holds one
folder per app plus `fixtures/`, and its layout matches the URLs:

```text
WebServer-dist/counter/hypernavigator.json  ->  http://127.0.0.1:8765/counter/hypernavigator.json
WebServer-dist/counter/index.html           ->  http://127.0.0.1:8765/counter/index.html
WebServer-dist/notes/hypernavigator.json    ->  http://127.0.0.1:8765/notes/hypernavigator.json
WebServer-dist/fixtures/<name>/...          ->  http://127.0.0.1:8765/fixtures/<name>/...
```

Versions: Kotlin 2.4.20 (with the matching Compose Compiler and serialization plugins), Compose
Multiplatform 1.12.1, Material 3 1.9.0, Lifecycle 2.11.0, Navigation Compose 2.9.2, Gradle 9.7.0. Needs JDK 17 or newer and Python 3. The first build downloads Gradle, Kotlin, Compose
and a Node.js toolchain.

## Build and serve

From this folder:

```bash
scripts/build-example.sh     # builds :counter and :notes, then stages each into ../WebServer-dist/
scripts/serve-example.sh     # keep running; serves http://127.0.0.1:8765/
scripts/smoke-test.sh        # optional checks (uses its own temporary port)
```

Then start HyperNavigator (`cd ../HyperNavigator-src && npm run tauri:dev`) and open
`http://127.0.0.1:8765/counter/hypernavigator.json` or `http://127.0.0.1:8765/notes/hypernavigator.json`.
Each app also works as a normal web page at `http://127.0.0.1:8765/<app>/index.html` in any browser
with WebAssembly GC support.

The same steps without the scripts, from the repository root:

```bash
(cd WebAppExamples-src && ./gradlew :counter:wasmJsBrowserDistribution :notes:wasmJsBrowserDistribution --console=plain)
for app in counter notes; do
  rm -rf WebServer-dist/$app && mkdir -p WebServer-dist/$app
  cp -R WebAppExamples-src/$app/build/dist/wasmJs/productionExecutable/. WebServer-dist/$app/
  cp WebAppExamples-src/$app/hypernavigator.json WebServer-dist/$app/
done
rm -rf WebServer-dist/fixtures && mkdir -p WebServer-dist/fixtures
cp -R WebAppExamples-src/fixtures/invalid-json WebAppExamples-src/fixtures/wrong-schema \
      WebAppExamples-src/fixtures/wrong-runtime WebAppExamples-src/fixtures/missing-entry \
      WebAppExamples-src/fixtures/escaping-entry WebServer-dist/fixtures/
python3 -m http.server 8765 --bind 127.0.0.1 --directory WebServer-dist
```

## Change an example

Edit `counter/src/wasmJsMain/kotlin/dev/hypernavigator/examples/counter/CounterApp.kt` (for example
`BUILD_LABEL`), run `scripts/build-example.sh` again and press **Reload** in HyperNavigator. The browser
does not need to be rebuilt.

## Add another example

1. Create `<name>/` with a `build.gradle.kts` like `counter/build.gradle.kts` (set `outputModuleName` and
   `outputFileName`), a `src/wasmJsMain/` tree and a `hypernavigator.json` whose `entry` is a relative
   `.html` file.
2. Add `include(":<name>")` to `settings.gradle.kts`.
3. Add `<name>` to `EXAMPLES` in `scripts/build-example.sh` (and a `check_app` line in
   `scripts/smoke-test.sh`), then run the script.
4. Open `http://127.0.0.1:8765/<name>/hypernavigator.json` in HyperNavigator.

## Porting Jetpack Compose code (`notes/`)

`notes/` is a small app written the way you would write it on Android today: Material 3
(`Scaffold`, `TopAppBar`, `LazyColumn`, `Card`, `OutlinedTextField`, `ExtendedFloatingActionButton`),
a `ViewModel` exposing `StateFlow`, `collectAsStateWithLifecycle`, type-safe Navigation Compose routes
(`@Serializable` + `toRoute()`), `rememberSaveable`, and string/plural resources. All of it lives in
`commonMain`; only `main()` is web-specific.

**Unchanged from Android:** every `androidx.compose.*`, `androidx.lifecycle.*` and
`androidx.navigation.*` import and call, `@OptIn(ExperimentalMaterial3Api::class)`, Kotlin
coroutines/`StateFlow`, `kotlinx.serialization` routes.

**What had to change:**

| Android | HyperNavigator (Compose Multiplatform, Kotlin/Wasm) |
|---|---|
| `class MainActivity { setContent { NotesApp() } }` | `fun main() = ComposeViewport("webApp") { NotesApp() }` in `wasmJsMain`, plus `index.html` and `hypernavigator.json` |
| Android Gradle plugin; Google `androidx.*` artifacts | Kotlin Multiplatform `wasmJs` target; JetBrains artifacts with the same packages (`org.jetbrains.compose.material3:material3`, `org.jetbrains.androidx.lifecycle:*`, `org.jetbrains.androidx.navigation:navigation-compose`) |
| `viewModel()` (reflection-based factory) | `viewModel { NotesViewModel() }` (explicit initializer; also valid on Android) |
| `res/values/strings.xml`, `R.string.x`, `androidx.compose.ui.res.stringResource` | `composeResources/values/strings.xml`, `Res.string.x`, `org.jetbrains.compose.resources.stringResource` / `pluralStringResource` (same XML format) |
| `Icons.Default.Add` (material-icons) | a `"+"` `Text` (the icons artifact was left out to keep dependencies small) |

**Not portable** (no equivalent on Kotlin/Wasm in v0.1): `Context`, `Intent`, `Toast`, runtime
permissions, `java.io.File`, Room/SQLite, CameraX, Maps Compose, Hilt, and other Android-only
libraries. Data here is in memory only; file access exists only through HyperNavigator's
"Share folder names".

To keep one codebase for both platforms, add `androidTarget()` (with the Android Gradle plugin and an
Android SDK) next to `wasmJs` in a Kotlin Multiplatform module: the `commonMain` code above is used
unchanged on Android.

## Talking to HyperNavigator

An app gets no HyperNavigator or Tauri API. The only Host-to-app channel in v0.1 is a one-way
`window` event, `hypernavigator:folder-listing`. Its `detail` is a JSON string:

```json
{ "type": "hypernavigator.folderListing", "version": 1, "truncated": false,
  "entries": [ { "name": "notes.md", "kind": "file" } ] }
```

`kind` is `file`, `directory`, `symlink` or `other`; at most 100 entries; no paths. See
`counter/src/wasmJsMain/kotlin/dev/hypernavigator/examples/counter/HostBridge.kt` for a Kotlin/Wasm
listener.
