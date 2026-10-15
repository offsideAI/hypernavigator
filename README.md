# hypernavigator

HyperNavigator developer preview (trusted localhost only). The product spec is in [PRD.md](PRD.md).

| Folder | What it is |
|---|---|
| [HyperNavigator-src/](HyperNavigator-src/README.md) | The browser: Tauri 2 + Rust host, TypeScript/Vite browser chrome. See also [IMPLEMENTATION_NOTES.md](HyperNavigator-src/IMPLEMENTATION_NOTES.md). |
| [WebAppExamples-src/](WebAppExamples-src/README.md) | Apps that run inside the browser: Kotlin/Wasm Compose Multiplatform examples (Gradle), served over HTTP. |
| `WebServer-dist/` | Generated web server root (git-ignored): the staged apps that `serve-example.sh` publishes. |

The two source folders are independent projects that meet only over HTTP at `http://127.0.0.1:8765`.

```bash
# terminal A
cd WebAppExamples-src && scripts/build-example.sh && scripts/serve-example.sh
# terminal B
cd HyperNavigator-src && npm install && npm run tauri:dev
```
