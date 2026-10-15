# Manifest fixtures

`scripts/build-example.sh` stages each folder under `WebServer-dist/fixtures/<name>/` (repo root), so with the example
server running you can open, for example,
`http://127.0.0.1:8765/fixtures/wrong-runtime/hypernavigator.json` in HyperNavigator and check that
it shows a clear error (HN-003 / HN-011). `scripts/smoke-test.sh` checks them over HTTP.

| Fixture | Expected error |
|---|---|
| `invalid-json` | Manifest is not valid JSON |
| `wrong-schema` | Unsupported schemaVersion 2 |
| `wrong-runtime` | Unsupported runtime "jvm-jar" |
| `missing-entry` | Manifest entry "index.html" is not available (HTTP 404) |
| `escaping-entry` | Manifest entry is invalid (`..` segment) |
