HyperNavigator: Proving the Concept with Compose Multiplatform

I would call it HyperNavigator POC-001: Remote Compose Rendering.
The objective is very specific:
Enter a URL into HyperNavigator, download a compiled Kotlin Compose application bundle, and render its @Composable UI inside the HyperNavigator window—not inside an APK, external Android runtime, or separately launched application window.

The demo should show a remotely supplied counter application with a functioning button. Clicking the button must update its UI through normal Compose recomposition.
HyperNavigator


http://localhost:8000/counter.jar



REMOTE KOTLIN COMPOSE APPLICATION
Hello from HyperNavigator!
This interface comes from the downloaded JAR.
0
 Increment counter
Interactive illustration of the POC's expected behavior.
Remote bundle loaded · Compose UI rendered in browser viewport


For this first experiment, we can use a trusted local HTTP server and a temporary in-process classloader. This is the absolute smallest way to validate that Kotlin Compose bytecode can be delivered remotely and participate in the Host's composition.
It is deliberately not a security sandbox. The POC must accept only developer-controlled bundles. We would move execution into a separate OS-sandboxed process before supporting untrusted remote applications.
3. HyperNavigator POC-001 — Source code
I created a small, three-module Kotlin/Gradle project implementing the experiment.
HyperNavigator POC-001
Kotlin source · Gradle project · ZIP
Download the complete source code



Includes the Compose desktop browser Host, shared SDK interface, remotely distributed counter application, HTTP bundle loader, and setup instructions.

The project is organized like this:
dotweb-poc-001/
├── build.gradle.kts
├── settings.gradle.kts
├── sdk/
│   └── HyperNavigatorApp.kt
├── plugin/
│   └── RemoteCounter.kt
└── host/
    ├── Main.kt
    └── BundleLoader.kt


The shared interface is deliberately minimal:

interface HyperNavigatorApp {
    @Composable
    fun Content()
}



The remote application implements that interface using regular Compose syntax, including remember, mutableIntStateOf, Column, Text, and Button.
The Host downloads the JAR using Java's HTTP client, reads its HyperNavigator-Entrypoint manifest attribute, instantiates the class with a URLClassLoader, and invokes its composable content inside the Host's viewport.
Because both modules use the same pinned Compose dependencies, they can share the rendering runtime.
Run it on macOS
Install JDK 21 and Gradle, extract the archive, then open two terminals in the project directory.
Terminal 1 — Build and serve the remote application

gradle :plugin:jar

python3 -m http.server 8000 \
  --directory plugin/build/libs



Terminal 2 — Launch HyperNavigator

gradle :host:run



When the browser window appears, navigate to:
http://localhost:8000/plugin-0.1.0.jar


Click Go. The counter interface should render directly inside HyperNavigator.
Then modify the counter application's Kotlin source, rebuild its JAR, and press Go again. The Host should load the new version without requiring a Host rebuild.
Verification status: The source files and ZIP structure were checked, but I couldn't run a Gradle build or GUI test in this environment because Gradle and external dependency downloads were unavailable. Compilation and execution still need to be verified on your machine.

4. What comes immediately after this POC?
The second experiment should introduce the custom Compose Applier and an IPC protocol.
The official Compose Runtime exposes Applier, AbstractApplier, and ComposeNode specifically for constructing and updating custom node trees. This is the foundation for separating execution of a composable function from rendering of its output. 

Android Developers
+1



The architecture would evolve as follows:
POC-001	POC-002
Download Kotlin JAR	Download Kotlin JAR
Load inside Host JVM	Load inside sandboxed JVM
Invoke @Composable directly	Invoke @Composable using custom Applier
Compose renders in Host	Serialize UI nodes over IPC
One process	Separate execution and rendering processes
Trusted code only	Foundation for untrusted code
For production, we would also need signed bundles, versioned APIs, OS-enforced filesystem permissions, network restrictions, and resource limits.
The decision I would make now is to validate POC-001 before modifying any Compose Multiplatform source. If we can download a separately compiled Kotlin Compose bundle, interact with its UI inside HyperNavigator, and replace it without rebuilding the browser, we've established the fundamental rendering and distribution model.
The next engineering milestone would then be moving that same functionality across a real process boundary without changing the application author's Compose programming model.

---
Yes. Compose Multiplatform is open-source enough to use as HyperNavigator's desktop rendering engine, including in a commercial, closed-source, or independently developed browser.
You do not need to fork Compose Multiplatform, negotiate a license with JetBrains, or write a new renderer.
1. What is open source?









JetBrains Compose Multiplatform
Apache 2.0
The framework, Gradle tooling, and desktop integration are publicly available. You can use, modify, fork, and commercially distribute the code, subject to the license's conditions. 

GitHub
+1















Compose Multiplatform Core
Apache 2.0
JetBrains maintains a public fork of the AndroidX Compose core codebase, including the runtime and UI implementations that support desktop rendering. 

GitHub















Skiko + Skia
Open-source
These supply the native graphics layer underneath Compose Desktop. Skiko is Apache 2.0 licensed; Skia uses a permissive BSD-style license. 

GitHub






Apache 2.0 allows HyperNavigator to use the code without publishing HyperNavigator's own source, though redistribution requires preserving applicable license and notice files. Bundled dependencies, fonts, and assets can carry additional license obligations.
There is another important advantage: the desktop implementation already supports JVM applications on macOS, Windows, and Linux, with hardware-accelerated rendering. 

GitHub



My recommendation is to depend on Compose Multiplatform as a library, not fork it. A fork would create a substantial maintenance obligation without helping us prove HyperNavigator's most important innovation: downloading Kotlin Compose applications and rendering them within a browser-controlled viewport.

2. The smallest proof of concept worth building
I would call it HyperNavigator POC-001: Remote Compose Rendering.
The objective is very specific:
Enter a URL into HyperNavigator, download a compiled Kotlin Compose application bundle, and render its @Composable UI inside the HyperNavigator window—not inside an APK, external Android runtime, or separately launched application window.

The demo should show a remotely supplied counter application with a functioning button. Clicking the button must update its UI through normal Compose recomposition.
HyperNavigator


http://localhost:8000/counter.jar



REMOTE KOTLIN COMPOSE APPLICATION
Hello from HyperNavigator!
This interface comes from the downloaded JAR.
0
 Increment counter
Interactive illustration of the POC's expected behavior.
Remote bundle loaded · Compose UI rendered in browser viewport


For this first experiment, we can use a trusted local HTTP server and a temporary in-process classloader. This is the absolute smallest way to validate that Kotlin Compose bytecode can be delivered remotely and participate in the Host's composition.
It is deliberately not a security sandbox. The POC must accept only developer-controlled bundles. We would move execution into a separate OS-sandboxed process before supporting untrusted remote applications.
3. HyperNavigator POC-001 — Source code
I created a small, three-module Kotlin/Gradle project implementing the experiment.
HyperNavigator POC-001
Kotlin source · Gradle project · ZIP
Download the complete source code



Includes the Compose desktop browser Host, shared SDK interface, remotely distributed counter application, HTTP bundle loader, and setup instructions.

The project is organized like this:
dotweb-poc-001/
├── build.gradle.kts
├── settings.gradle.kts
├── sdk/
│   └── HyperNavigatorApp.kt
├── plugin/
│   └── RemoteCounter.kt
└── host/
    ├── Main.kt
    └── BundleLoader.kt


The shared interface is deliberately minimal:

interface HyperNavigatorApp {
    @Composable
    fun Content()
}



The remote application implements that interface using regular Compose syntax, including remember, mutableIntStateOf, Column, Text, and Button.
The Host downloads the JAR using Java's HTTP client, reads its HyperNavigator-Entrypoint manifest attribute, instantiates the class with a URLClassLoader, and invokes its composable content inside the Host's viewport.
Because both modules use the same pinned Compose dependencies, they can share the rendering runtime.
Run it on macOS
Install JDK 21 and Gradle, extract the archive, then open two terminals in the project directory.
Terminal 1 — Build and serve the remote application

gradle :plugin:jar

python3 -m http.server 8000 \
  --directory plugin/build/libs



Terminal 2 — Launch HyperNavigator

gradle :host:run



When the browser window appears, navigate to:
http://localhost:8000/plugin-0.1.0.jar


Click Go. The counter interface should render directly inside HyperNavigator.
Then modify the counter application's Kotlin source, rebuild its JAR, and press Go again. The Host should load the new version without requiring a Host rebuild.
Verification status: The source files and ZIP structure were checked, but I couldn't run a Gradle build or GUI test in this environment because Gradle and external dependency downloads were unavailable. Compilation and execution still need to be verified on your machine.

4. What comes immediately after this POC?
The second experiment should introduce the custom Compose Applier and an IPC protocol.
The official Compose Runtime exposes Applier, AbstractApplier, and ComposeNode specifically for constructing and updating custom node trees. This is the foundation for separating execution of a composable function from rendering of its output. 

Android Developers
+1



The architecture would evolve as follows:
POC-001	POC-002
Download Kotlin JAR	Download Kotlin JAR
Load inside Host JVM	Load inside sandboxed JVM
Invoke @Composable directly	Invoke @Composable using custom Applier
Compose renders in Host	Serialize UI nodes over IPC
One process	Separate execution and rendering processes
Trusted code only	Foundation for untrusted code
For production, we would also need signed bundles, versioned APIs, OS-enforced filesystem permissions, network restrictions, and resource limits.
The decision I would make now is to validate POC-001 before modifying any Compose Multiplatform source. If we can download a separately compiled Kotlin Compose bundle, interact with its UI inside HyperNavigator, and replace it without rebuilding the browser, we've established the fundamental rendering and distribution model.
The next engineering milestone would then be moving that same functionality across a real process boundary without changing the application author's Compose programming model.
















[Web search](chatgpt-system-hint://search) We will rename this HyperNavigator and not HyperNavigator. Can you create the PRD.md for HyperNavigator desktop app and tech stack and create a prompt for Claude Code so that it will implement the MVP of HyperNavigator (probably using Rust + Tauri)














Worked for 5m 10s






HyperNavigator — MVP architecture and product specification
I recommend building HyperNavigator with Rust + Tauri 2 for the desktop shell and Kotlin/Wasm + Compose Multiplatform for remotely delivered applications.
There is one important architectural distinction: Tauri uses the operating system's WebView, not the Compose Desktop/JVM renderer. Fortunately, Compose Multiplatform has a Kotlin/Wasm target that renders real @Composable interfaces onto an HTML canvas through ComposeViewport. This lets HyperNavigator display actual Compose applications inside its browser window without APKs, an Android emulator, or translating Compose components into HTML manually. 

Tauri
+2



Selected technology stack
Layer	Technology
Desktop application	Rust + Tauri 2
Browser chrome	TypeScript + Vite
Embedded application viewport	Separate Tauri child WebView
Remote application language	Kotlin with Compose Multiplatform
Remote application compilation	Kotlin/Wasm (wasmJs)
UI rendering	Compose for Web + canvas
Application distribution	HTTP/HTTPS static bundles
Host capabilities	Rust-mediated permissions
Initial platform	macOS Apple Silicon
Future platforms	Windows and Linux
The key implementation detail is to use two WebViews within one native Tauri window: one trusted WebView for the address bar and browser controls, and one separate WebView for the downloaded Compose application.
Tauri supports this through Window::add_child and WebviewBuilder, although the multi-WebView API is currently marked unstable. This is a reasonable dependency for an experimental MVP, but should be recorded as an engineering risk. 

Rust
+1



It also avoids placing downloaded application code directly inside a privileged browser-shell iframe. Tauri has documented iframe-related security boundaries that deserve particular care when the product's purpose is eventually to execute untrusted code. 

Tauri
+1



MVP success criterion: Navigate to a locally served HyperNavigator application bundle, render a functioning Kotlin Compose counter inside the browser viewport, reload it, and demonstrate a narrowly scoped, user-approved filesystem capability.
No APKs, Android Activities, or JVM JAR execution are required for this MVP.
