//! `hypernavigator.json` v1: parsing, validation, and bounded fetching over loopback HTTP.

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::policy::{OriginPolicy, PolicyError};

pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const SCHEMA_VERSION: u64 = 1;
pub const RUNTIME_COMPOSE_WASM_WEB: &str = "compose-wasm-web";
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

const MAX_ID_LEN: usize = 128;
const MAX_NAME_CHARS: usize = 64;
const MAX_VERSION_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u64,
    pub id: String,
    pub name: String,
    pub version: String,
    pub runtime: String,
    pub entry: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    TooLarge,
    NotUtf8,
    InvalidJson(String),
    NotAnObject,
    MissingSchemaVersion,
    UnsupportedSchemaVersion(String),
    SchemaMismatch(String),
    UnsupportedRuntime(String),
    InvalidField(&'static str, &'static str),
    PermissionsNotSupported,
    Entry(PolicyError),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "Manifest is larger than {} KiB", MAX_MANIFEST_BYTES / 1024),
            Self::NotUtf8 => write!(f, "Manifest is not valid UTF-8"),
            Self::InvalidJson(e) => write!(f, "Manifest is not valid JSON: {e}"),
            Self::NotAnObject => write!(f, "Manifest must be a JSON object"),
            Self::MissingSchemaVersion => write!(f, "Manifest is missing \"schemaVersion\""),
            Self::UnsupportedSchemaVersion(v) => write!(f, "Unsupported schemaVersion {v} (this build supports {SCHEMA_VERSION})"),
            Self::SchemaMismatch(e) => write!(f, "Manifest does not match schema v{SCHEMA_VERSION}: {e}"),
            Self::UnsupportedRuntime(r) => write!(f, "Unsupported runtime \"{r}\" (expected \"{RUNTIME_COMPOSE_WASM_WEB}\")"),
            Self::InvalidField(field, why) => write!(f, "Manifest field \"{field}\" {why}"),
            Self::PermissionsNotSupported => write!(f, "Manifest \"permissions\" must be empty in v0.1; access is granted interactively by the user"),
            Self::Entry(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Parses and validates a manifest body. Does not resolve `entry`; see [`OriginPolicy::resolve_entry`].
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(ManifestError::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ManifestError::NotUtf8)?;
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| ManifestError::InvalidJson(e.to_string()))?;
    let object = value.as_object().ok_or(ManifestError::NotAnObject)?;

    // Check the version before the full schema so the error names the real problem.
    match object.get("schemaVersion") {
        None => return Err(ManifestError::MissingSchemaVersion),
        Some(v) if v.as_u64() == Some(SCHEMA_VERSION) => {}
        Some(v) => return Err(ManifestError::UnsupportedSchemaVersion(v.to_string())),
    }

    let manifest: Manifest = serde_json::from_value(value).map_err(|e| ManifestError::SchemaMismatch(e.to_string()))?;
    if manifest.runtime != RUNTIME_COMPOSE_WASM_WEB {
        return Err(ManifestError::UnsupportedRuntime(truncate_for_display(&manifest.runtime)));
    }
    validate_id(&manifest.id)?;
    validate_name(&manifest.name)?;
    validate_version(&manifest.version)?;
    if !manifest.permissions.is_empty() {
        return Err(ManifestError::PermissionsNotSupported);
    }
    Ok(manifest)
}

fn validate_id(id: &str) -> Result<(), ManifestError> {
    let valid = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id.starts_with(|c: char| c.is_ascii_alphanumeric())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(ManifestError::InvalidField("id", "must be 1-128 ASCII letters, digits, '.', '_' or '-'"))
    }
}

fn validate_name(name: &str) -> Result<(), ManifestError> {
    let count = name.chars().count();
    if name.trim().is_empty() || count > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        Err(ManifestError::InvalidField("name", "must be 1-64 printable characters"))
    } else {
        Ok(())
    }
}

/// Semver-like: MAJOR.MINOR.PATCH with optional `-prerelease` / `+build` of [0-9A-Za-z.-].
fn validate_version(version: &str) -> Result<(), ManifestError> {
    let invalid = ManifestError::InvalidField("version", "must look like MAJOR.MINOR.PATCH (semver)");
    if version.is_empty() || version.len() > MAX_VERSION_LEN {
        return Err(invalid);
    }
    let (core, suffix) = match version.find(['-', '+']) {
        Some(i) => (&version[..i], &version[i + 1..]),
        None => (version, ""),
    };
    let numbers: Vec<&str> = core.split('.').collect();
    let core_ok = numbers.len() == 3 && numbers.iter().all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    let suffix_ok = suffix.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
        && (version.len() == core.len() || !suffix.is_empty());
    if core_ok && suffix_ok {
        Ok(())
    } else {
        Err(invalid)
    }
}

fn truncate_for_display(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(64).collect()
}

/// A validated manifest plus the URLs it resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedApp {
    pub manifest_url: Url,
    pub entry_url: Url,
    pub manifest: Manifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    Policy(PolicyError),
    ServerUnavailable(String),
    Timeout,
    Redirect(Option<String>),
    HttpStatus(u16),
    NotFound,
    ContentType(String),
    Manifest(ManifestError),
    EntryMissing(String, u16),
    Network(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy(e) => write!(f, "{e}"),
            Self::ServerUnavailable(origin) => write!(
                f,
                "Could not connect to {origin}. Is the example server running? (WebAppExamples-src/scripts/serve-example.sh)"
            ),
            Self::Timeout => write!(f, "The server did not respond within {} seconds", FETCH_TIMEOUT.as_secs()),
            Self::Redirect(Some(location)) => write!(
                f,
                "The server answered with a redirect to \"{}\"; redirects are not followed in this developer preview",
                truncate_for_display(location)
            ),
            Self::Redirect(None) => write!(f, "The server answered with a redirect; redirects are not followed"),
            Self::HttpStatus(code) => write!(f, "The server answered HTTP {code} for the manifest"),
            Self::NotFound => write!(f, "No hypernavigator.json at that URL (HTTP 404)"),
            Self::ContentType(ct) => write!(f, "Manifest Content-Type must be application/json, got \"{}\"", truncate_for_display(ct)),
            Self::Manifest(e) => write!(f, "{e}"),
            Self::EntryMissing(path, code) => write!(f, "Manifest entry \"{path}\" is not available (HTTP {code})"),
            Self::Network(e) => write!(f, "Network error: {}", truncate_for_display(e)),
        }
    }
}

impl std::error::Error for FetchError {}

impl From<ManifestError> for FetchError {
    fn from(e: ManifestError) -> Self {
        Self::Manifest(e)
    }
}

impl From<PolicyError> for FetchError {
    fn from(e: PolicyError) -> Self {
        Self::Policy(e)
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(FETCH_TIMEOUT))
        // 0 means "never follow": 3xx responses are returned to us and rejected below.
        .max_redirects(0)
        .http_status_as_error(false)
        // Loopback only; never route through an environment-configured proxy.
        .proxy(None)
        // No connection reuse: dev servers such as `python3 -m http.server` speak HTTP/1.0 and close
        // after each response, so a pooled connection can be dead by the time the entry HEAD runs.
        .max_idle_connections(0)
        .max_idle_connections_per_host(0)
        .build()
        .into()
}

fn map_transport_error(policy: &OriginPolicy, e: ureq::Error) -> FetchError {
    match e {
        ureq::Error::Timeout(_) => FetchError::Timeout,
        ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => FetchError::ServerUnavailable(policy.origin()),
        ureq::Error::Io(io) if matches!(io.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::ConnectionReset) => {
            FetchError::ServerUnavailable(policy.origin())
        }
        ureq::Error::BodyExceedsLimit(_) => FetchError::Manifest(ManifestError::TooLarge),
        other => FetchError::Network(other.to_string()),
    }
}

fn reject_redirect(status: u16, location: Option<&str>) -> Result<(), FetchError> {
    if (300..400).contains(&status) {
        Err(FetchError::Redirect(location.map(str::to_string)))
    } else {
        Ok(())
    }
}

/// Fetches, parses and validates the manifest, resolves its entry, and checks the entry exists.
/// Blocking; call from a background thread.
pub fn fetch_app(policy: &OriginPolicy, manifest_url: &Url) -> Result<ResolvedApp, FetchError> {
    if !policy.is_allowed_origin(manifest_url) {
        return Err(FetchError::Policy(PolicyError::BlockedHost(manifest_url.host_str().unwrap_or("").to_string())));
    }
    let agent = agent();

    let mut response = agent
        .get(manifest_url.as_str())
        .header("Accept", "application/json")
        .call()
        .map_err(|e| map_transport_error(policy, e))?;
    let status = response.status().as_u16();
    reject_redirect(status, response.headers().get("location").and_then(|v| v.to_str().ok()))?;
    match status {
        200 => {}
        404 => return Err(FetchError::NotFound),
        other => return Err(FetchError::HttpStatus(other)),
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let mime = content_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if mime != "application/json" {
        return Err(FetchError::ContentType(content_type));
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_MANIFEST_BYTES as u64)
        .read_to_vec()
        .map_err(|e| map_transport_error(policy, e))?;

    let manifest = parse_manifest(&body)?;
    let entry_url = policy.resolve_entry(manifest_url, &manifest.entry).map_err(ManifestError::Entry)?;

    // HEAD the entry so a missing build shows a clear error instead of a blank guest.
    let entry_response = agent.head(entry_url.as_str()).call().map_err(|e| map_transport_error(policy, e))?;
    let entry_status = entry_response.status().as_u16();
    reject_redirect(entry_status, entry_response.headers().get("location").and_then(|v| v.to_str().ok()))?;
    if entry_status != 200 {
        return Err(FetchError::EntryMissing(manifest.entry.clone(), entry_status));
    }

    Ok(ResolvedApp { manifest_url: manifest_url.clone(), entry_url, manifest })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, TcpListener};
    use std::thread;

    const VALID: &str = r#"{
        "schemaVersion": 1,
        "id": "dev.hypernavigator.examples.counter",
        "name": "Compose Counter",
        "version": "0.1.0",
        "runtime": "compose-wasm-web",
        "entry": "index.html",
        "permissions": []
    }"#;

    fn with(field: &str, value: &str) -> String {
        let mut v: serde_json::Value = serde_json::from_str(VALID).unwrap();
        v[field] = serde_json::from_str(value).unwrap();
        v.to_string()
    }

    #[test]
    fn parses_valid_manifest() {
        let m = parse_manifest(VALID.as_bytes()).unwrap();
        assert_eq!(m.id, "dev.hypernavigator.examples.counter");
        assert_eq!(m.entry, "index.html");
        // `permissions` is optional.
        let mut v: serde_json::Value = serde_json::from_str(VALID).unwrap();
        v.as_object_mut().unwrap().remove("permissions");
        assert!(parse_manifest(v.to_string().as_bytes()).is_ok());
    }

    #[test]
    fn resolves_the_default_entry() {
        let m = parse_manifest(VALID.as_bytes()).unwrap();
        let url = Url::parse("http://127.0.0.1:8765/counter/hypernavigator.json").unwrap();
        assert_eq!(
            OriginPolicy::DEV.resolve_entry(&url, &m.entry).unwrap().as_str(),
            "http://127.0.0.1:8765/counter/index.html"
        );
    }

    #[test]
    fn rejects_invalid_json_and_encoding() {
        assert!(matches!(parse_manifest(b"{ not json"), Err(ManifestError::InvalidJson(_))));
        assert!(matches!(parse_manifest(b""), Err(ManifestError::InvalidJson(_))));
        assert_eq!(parse_manifest(b"[1,2]"), Err(ManifestError::NotAnObject));
        assert_eq!(parse_manifest(&[0xff, 0xfe, b'{', b'}']), Err(ManifestError::NotUtf8));
    }

    #[test]
    fn rejects_oversized_manifest() {
        let mut big = String::from(VALID);
        big.push_str(&" ".repeat(MAX_MANIFEST_BYTES));
        assert_eq!(parse_manifest(big.as_bytes()), Err(ManifestError::TooLarge));
    }

    #[test]
    fn rejects_version_and_runtime_mismatch() {
        assert_eq!(parse_manifest(with("schemaVersion", "2").as_bytes()), Err(ManifestError::UnsupportedSchemaVersion("2".into())));
        assert_eq!(parse_manifest(with("schemaVersion", "\"1\"").as_bytes()), Err(ManifestError::UnsupportedSchemaVersion("\"1\"".into())));
        assert_eq!(parse_manifest(with("schemaVersion", "1.5").as_bytes()), Err(ManifestError::UnsupportedSchemaVersion("1.5".into())));
        assert_eq!(parse_manifest(br#"{"id":"x"}"#), Err(ManifestError::MissingSchemaVersion));
        assert_eq!(
            parse_manifest(with("runtime", "\"jvm-jar\"").as_bytes()),
            Err(ManifestError::UnsupportedRuntime("jvm-jar".into()))
        );
    }

    #[test]
    fn rejects_schema_mismatch() {
        assert!(matches!(parse_manifest(with("extra", "true").as_bytes()), Err(ManifestError::SchemaMismatch(_))));
        assert!(matches!(parse_manifest(with("entry", "42").as_bytes()), Err(ManifestError::SchemaMismatch(_))));
        let mut v: serde_json::Value = serde_json::from_str(VALID).unwrap();
        v.as_object_mut().unwrap().remove("entry");
        assert!(matches!(parse_manifest(v.to_string().as_bytes()), Err(ManifestError::SchemaMismatch(_))));
    }

    #[test]
    fn validates_bounded_fields() {
        for bad_id in ["\"\"", "\"-leading\"", "\"has space\"", "\"a/b\""] {
            assert!(matches!(parse_manifest(with("id", bad_id).as_bytes()), Err(ManifestError::InvalidField("id", _))), "{bad_id}");
        }
        let long_id = format!("\"{}\"", "a".repeat(129));
        assert!(parse_manifest(with("id", &long_id).as_bytes()).is_err());
        for bad_name in ["\"\"", "\"   \"", "\"tab\\there\""] {
            assert!(matches!(parse_manifest(with("name", bad_name).as_bytes()), Err(ManifestError::InvalidField("name", _))), "{bad_name}");
        }
        let long_name = format!("\"{}\"", "n".repeat(65));
        assert!(parse_manifest(with("name", &long_name).as_bytes()).is_err());
        for good in ["\"1.0.0\"", "\"0.1.0-alpha.1\"", "\"2.3.4+build.5\"", "\"1.0.0-rc.1+sha.abc\""] {
            assert!(parse_manifest(with("version", good).as_bytes()).is_ok(), "{good}");
        }
        for bad in ["\"1.0\"", "\"v1.0.0\"", "\"1.0.0-\"", "\"1..0\"", "\"1.0.0 beta\"", "\"\""] {
            assert!(matches!(parse_manifest(with("version", bad).as_bytes()), Err(ManifestError::InvalidField("version", _))), "{bad}");
        }
        assert_eq!(parse_manifest(with("permissions", "[\"fs\"]").as_bytes()), Err(ManifestError::PermissionsNotSupported));
    }

    /// Minimal one-shot-per-connection HTTP server for fetch tests.
    fn serve(responses: Vec<(&'static str, String, String, Vec<u8>)>) -> (OriginPolicy, Url) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            for (status, content_type, extra_headers, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else { return };
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let is_head = buf.starts_with(b"HEAD");
                if !is_head {
                    let _ = stream.write_all(&body);
                }
            }
        });
        let policy = OriginPolicy { host: Ipv4Addr::LOCALHOST, port };
        let url = Url::parse(&format!("http://127.0.0.1:{port}/counter/hypernavigator.json")).unwrap();
        (policy, url)
    }

    fn json_ok(body: &str) -> (&'static str, String, String, Vec<u8>) {
        ("200 OK", "application/json".into(), String::new(), body.as_bytes().to_vec())
    }

    fn html_ok() -> (&'static str, String, String, Vec<u8>) {
        ("200 OK", "text/html".into(), String::new(), b"<html></html>".to_vec())
    }

    #[test]
    fn fetches_valid_app_and_checks_entry() {
        let (policy, url) = serve(vec![json_ok(VALID), html_ok()]);
        let app = fetch_app(&policy, &url).unwrap();
        assert_eq!(app.entry_url.path(), "/counter/index.html");
        assert_eq!(app.manifest.name, "Compose Counter");
    }

    /// Like `python3 -m http.server`: HTTP/1.0, no Connection header, closes each connection after responding.
    fn serve_http10(responses: Vec<(&'static str, &'static str, Vec<u8>)>) -> (OriginPolicy, Url) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            for (status, content_type, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else { return };
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let head = format!("HTTP/1.0 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n", body.len());
                let _ = stream.write_all(head.as_bytes());
                if !buf[..n].starts_with(b"HEAD") {
                    let _ = stream.write_all(&body);
                }
                // Close shortly after responding, as HTTP/1.0 servers do. A client that reuses
                // this connection writes its next request into a socket that is about to close.
                thread::spawn(move || {
                    thread::sleep(std::time::Duration::from_millis(100));
                    drop(stream);
                });
            }
        });
        let policy = OriginPolicy { host: Ipv4Addr::LOCALHOST, port };
        (policy, Url::parse(&format!("http://127.0.0.1:{port}/counter/hypernavigator.json")).unwrap())
    }

    #[test]
    fn works_with_http10_servers_that_close_connections() {
        for _ in 0..3 {
            let (policy, url) = serve_http10(vec![
                ("200 OK", "application/json", VALID.as_bytes().to_vec()),
                ("200 OK", "text/html", b"<html></html>".to_vec()),
            ]);
            assert!(fetch_app(&policy, &url).is_ok());
        }
    }

    #[test]
    fn rejects_missing_entry() {
        let (policy, url) = serve(vec![json_ok(VALID), ("404 Not Found", "text/html".into(), String::new(), vec![])]);
        assert_eq!(fetch_app(&policy, &url), Err(FetchError::EntryMissing("index.html".into(), 404)));
    }

    #[test]
    fn rejects_redirects_same_or_cross_origin() {
        for location in ["http://evil.example/hypernavigator.json", "/other/hypernavigator.json"] {
            let (policy, url) = serve(vec![(
                "302 Found",
                "text/html".into(),
                format!("Location: {location}\r\n"),
                vec![],
            )]);
            assert_eq!(fetch_app(&policy, &url), Err(FetchError::Redirect(Some(location.into()))));
        }
    }

    #[test]
    fn rejects_redirecting_entry() {
        let (policy, url) = serve(vec![
            json_ok(VALID),
            ("301 Moved Permanently", "text/html".into(), "Location: http://evil.example/\r\n".into(), vec![]),
        ]);
        assert!(matches!(fetch_app(&policy, &url), Err(FetchError::Redirect(_))));
    }

    #[test]
    fn rejects_oversized_response_body() {
        let mut body = String::from(VALID);
        body.push_str(&" ".repeat(MAX_MANIFEST_BYTES + 10));
        let (policy, url) = serve(vec![json_ok(&body)]);
        assert_eq!(fetch_app(&policy, &url), Err(FetchError::Manifest(ManifestError::TooLarge)));
    }

    #[test]
    fn rejects_wrong_content_type_and_status() {
        let (policy, url) = serve(vec![("200 OK", "text/html".into(), String::new(), VALID.as_bytes().to_vec())]);
        assert!(matches!(fetch_app(&policy, &url), Err(FetchError::ContentType(_))));
        let (policy, url) = serve(vec![("404 Not Found", "text/html".into(), String::new(), vec![])]);
        assert_eq!(fetch_app(&policy, &url), Err(FetchError::NotFound));
        let (policy, url) = serve(vec![("500 Internal Server Error", "text/plain".into(), String::new(), vec![])]);
        assert_eq!(fetch_app(&policy, &url), Err(FetchError::HttpStatus(500)));
    }

    #[test]
    fn rejects_invalid_manifest_over_http() {
        let (policy, url) = serve(vec![json_ok("{ nope")]);
        assert!(matches!(fetch_app(&policy, &url), Err(FetchError::Manifest(ManifestError::InvalidJson(_)))));
        let (policy, url) = serve(vec![json_ok(&with("entry", "\"../escape.html\""))]);
        assert!(matches!(fetch_app(&policy, &url), Err(FetchError::Manifest(ManifestError::Entry(_)))));
    }

    #[test]
    fn reports_offline_server() {
        // Bind then drop to get a port with nothing listening.
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap().local_addr().unwrap().port();
        let policy = OriginPolicy { host: Ipv4Addr::LOCALHOST, port };
        let url = Url::parse(&format!("http://127.0.0.1:{port}/hypernavigator.json")).unwrap();
        assert_eq!(fetch_app(&policy, &url), Err(FetchError::ServerUnavailable(policy.origin())));
    }

    #[test]
    fn refuses_to_fetch_other_origins() {
        let url = Url::parse("http://127.0.0.1:9/hypernavigator.json").unwrap();
        assert!(matches!(fetch_app(&OriginPolicy::DEV, &url), Err(FetchError::Policy(_))));
    }
}
