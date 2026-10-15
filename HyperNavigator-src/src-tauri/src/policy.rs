//! Local development origin policy.
//!
//! HyperNavigator v0.1 accepts exactly one origin, `http://127.0.0.1:8765`. The checks below run on
//! the raw user input as well as on the parsed URL, because the `url` crate normalizes aliases such
//! as `http://2130706433:8765/` or `http://127.1:8765/` to `127.0.0.1`, decodes `%2e` dot segments
//! and rewrites backslashes. Those aliases are rejected rather than silently accepted.

use std::fmt;
use std::net::Ipv4Addr;

use url::{Host, Url};

pub const MAX_URL_LEN: usize = 2048;
pub const MAX_ENTRY_LEN: usize = 256;
pub const MANIFEST_FILE_NAME: &str = "hypernavigator.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    Empty,
    TooLong,
    InvalidCharacters,
    Malformed,
    BlockedScheme(String),
    UserInfo,
    BlockedHost(String),
    BlockedPort(Option<u16>),
    NonCanonicalOrigin,
    QueryOrFragment,
    ForbiddenPathCharacters,
    Traversal,
    NotAManifestPath,
    InvalidEntry(&'static str),
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let allowed = OriginPolicy::DEV.origin();
        match self {
            Self::Empty => write!(f, "Enter a manifest URL, e.g. {allowed}/counter/hypernavigator.json"),
            Self::TooLong => write!(f, "URL is longer than {MAX_URL_LEN} characters"),
            Self::InvalidCharacters => write!(f, "URL contains whitespace, control characters or backslashes"),
            Self::Malformed => write!(f, "Not a valid URL"),
            Self::BlockedScheme(s) => write!(f, "Blocked scheme \"{s}:\" - only {allowed} is allowed in this developer preview"),
            Self::UserInfo => write!(f, "URLs with user names or passwords are not allowed"),
            Self::BlockedHost(h) => write!(f, "Blocked host \"{h}\" - only {allowed} is allowed in this developer preview"),
            Self::BlockedPort(Some(p)) => write!(f, "Blocked port {p} - only {allowed} is allowed"),
            Self::BlockedPort(None) => write!(f, "Missing port - only {allowed} is allowed"),
            Self::NonCanonicalOrigin => write!(f, "Write the origin exactly as {allowed} (aliases are not accepted)"),
            Self::QueryOrFragment => write!(f, "Manifest URLs must not contain a query or fragment"),
            Self::ForbiddenPathCharacters => write!(f, "Path may only contain letters, digits, '.', '_', '~', '-' and '/' (no percent-encoding)"),
            Self::Traversal => write!(f, "Path must not contain empty, '.' or '..' segments"),
            Self::NotAManifestPath => write!(f, "URL must point to a {MANIFEST_FILE_NAME} file (or a folder containing one)"),
            Self::InvalidEntry(why) => write!(f, "Manifest entry is invalid: {why}"),
        }
    }
}

impl std::error::Error for PolicyError {}

/// The single origin HyperNavigator talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OriginPolicy {
    pub host: Ipv4Addr,
    pub port: u16,
}

impl OriginPolicy {
    pub const DEV: OriginPolicy = OriginPolicy { host: Ipv4Addr::new(127, 0, 0, 1), port: 8765 };

    /// `http://127.0.0.1:8765` (no trailing slash).
    pub fn origin(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    /// Exact scheme, IPv4 host, port, and no userinfo.
    pub fn is_allowed_origin(&self, url: &Url) -> bool {
        url.scheme() == "http"
            && url.username().is_empty()
            && url.password().is_none()
            && url.host() == Some(Host::Ipv4(self.host))
            && url.port() == Some(self.port)
    }

    /// Guest top-level and frame navigations: any path on the allowed origin.
    pub fn allows_guest_navigation(&self, url: &Url) -> bool {
        self.is_allowed_origin(url)
    }

    /// Validates user input from the address bar and returns the canonical manifest URL.
    /// A URL ending in `/` gets `hypernavigator.json` appended.
    pub fn parse_manifest_url(&self, input: &str) -> Result<Url, PolicyError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(PolicyError::Empty);
        }
        if input.len() > MAX_URL_LEN {
            return Err(PolicyError::TooLong);
        }
        if input.chars().any(|c| c.is_control() || c.is_whitespace() || c == '\\') {
            return Err(PolicyError::InvalidCharacters);
        }

        // Parse first so the user gets a specific reason (scheme, host, port, ...).
        let parsed = Url::parse(input).map_err(|_| PolicyError::Malformed)?;
        if parsed.scheme() != "http" {
            return Err(PolicyError::BlockedScheme(parsed.scheme().to_string()));
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(PolicyError::UserInfo);
        }
        match parsed.host() {
            Some(Host::Ipv4(ip)) if ip == self.host => {}
            Some(host) => return Err(PolicyError::BlockedHost(host.to_string())),
            None => return Err(PolicyError::Malformed),
        }
        if parsed.port() != Some(self.port) {
            return Err(PolicyError::BlockedPort(parsed.port()));
        }

        // Then require the canonical spelling in the raw input, rejecting normalized aliases.
        let origin = self.origin();
        let rest = input.strip_prefix(origin.as_str()).ok_or(PolicyError::NonCanonicalOrigin)?;
        if !(rest.is_empty() || rest.starts_with('/')) {
            return Err(PolicyError::NonCanonicalOrigin);
        }
        if rest.contains('?') || rest.contains('#') {
            return Err(PolicyError::QueryOrFragment);
        }

        let mut path = if rest.is_empty() { "/".to_string() } else { rest.to_string() };
        check_path_characters(&path)?;
        if path.ends_with('/') {
            path.push_str(MANIFEST_FILE_NAME);
        }
        check_segments(path.strip_prefix('/').unwrap_or(&path))?;
        if path.rsplit('/').next() != Some(MANIFEST_FILE_NAME) {
            return Err(PolicyError::NotAManifestPath);
        }

        let url = Url::parse(&format!("{origin}{path}")).map_err(|_| PolicyError::Malformed)?;
        debug_assert!(self.is_allowed_origin(&url));
        Ok(url)
    }

    /// Resolves a manifest `entry` (relative path) against the manifest URL. The result must stay on
    /// the same origin and inside the manifest's directory.
    pub fn resolve_entry(&self, manifest_url: &Url, entry: &str) -> Result<Url, PolicyError> {
        if entry.is_empty() || entry.len() > MAX_ENTRY_LEN {
            return Err(PolicyError::InvalidEntry("must be 1-256 characters"));
        }
        if entry.starts_with('/') {
            return Err(PolicyError::InvalidEntry("must be a relative path"));
        }
        if !entry.chars().all(is_safe_path_char) {
            return Err(PolicyError::InvalidEntry(
                "may only contain letters, digits, '.', '_', '~', '-' and '/' (no scheme, query, or percent-encoding)",
            ));
        }
        check_segments(entry).map_err(|_| PolicyError::InvalidEntry("must not contain empty, '.' or '..' segments"))?;
        if !entry.ends_with(".html") {
            return Err(PolicyError::InvalidEntry("must be an .html file"));
        }

        let resolved = manifest_url.join(entry).map_err(|_| PolicyError::InvalidEntry("cannot be resolved"))?;
        let base_dir = &manifest_url.path()[..=manifest_url.path().rfind('/').unwrap_or(0)];
        if !self.is_allowed_origin(&resolved) || !resolved.path().starts_with(base_dir) {
            return Err(PolicyError::InvalidEntry("resolves outside the manifest's origin or folder"));
        }
        Ok(resolved)
    }
}

fn is_safe_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '~' | '-' | '/')
}

fn check_path_characters(path: &str) -> Result<(), PolicyError> {
    if path.chars().all(is_safe_path_char) {
        Ok(())
    } else {
        Err(PolicyError::ForbiddenPathCharacters)
    }
}

/// `relative` has no leading slash; every segment must be non-empty and not `.`/`..`.
fn check_segments(relative: &str) -> Result<(), PolicyError> {
    if relative.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        Err(PolicyError::Traversal)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: OriginPolicy = OriginPolicy::DEV;

    fn ok(input: &str) -> String {
        P.parse_manifest_url(input).unwrap_or_else(|e| panic!("{input}: {e}")).to_string()
    }

    fn err(input: &str) -> PolicyError {
        P.parse_manifest_url(input).expect_err(input)
    }

    #[test]
    fn accepts_canonical_manifest_urls() {
        assert_eq!(ok("http://127.0.0.1:8765/counter/hypernavigator.json"), "http://127.0.0.1:8765/counter/hypernavigator.json");
        assert_eq!(ok("  http://127.0.0.1:8765/counter/hypernavigator.json  "), "http://127.0.0.1:8765/counter/hypernavigator.json");
        assert_eq!(ok("http://127.0.0.1:8765/counter/"), "http://127.0.0.1:8765/counter/hypernavigator.json");
        assert_eq!(ok("http://127.0.0.1:8765"), "http://127.0.0.1:8765/hypernavigator.json");
        assert_eq!(ok("http://127.0.0.1:8765/a/b-c_d.e~f/hypernavigator.json"), "http://127.0.0.1:8765/a/b-c_d.e~f/hypernavigator.json");
    }

    #[test]
    fn rejects_blocked_schemes() {
        assert_eq!(err("https://127.0.0.1:8765/hypernavigator.json"), PolicyError::BlockedScheme("https".into()));
        assert_eq!(err("file:///etc/passwd"), PolicyError::BlockedScheme("file".into()));
        assert_eq!(err("data:text/html,<b>x</b>"), PolicyError::BlockedScheme("data".into()));
        assert_eq!(err("javascript:alert(1)"), PolicyError::BlockedScheme("javascript".into()));
        assert_eq!(err("ftp://127.0.0.1:8765/hypernavigator.json"), PolicyError::BlockedScheme("ftp".into()));
    }

    #[test]
    fn rejects_other_hosts_and_aliases() {
        assert!(matches!(err("http://example.com:8765/hypernavigator.json"), PolicyError::BlockedHost(_)));
        assert!(matches!(err("http://localhost:8765/hypernavigator.json"), PolicyError::BlockedHost(_)));
        assert!(matches!(err("http://[::1]:8765/hypernavigator.json"), PolicyError::BlockedHost(_)));
        assert!(matches!(err("http://[::ffff:127.0.0.1]:8765/hypernavigator.json"), PolicyError::BlockedHost(_)));
        assert!(matches!(err("http://127.0.0.2:8765/hypernavigator.json"), PolicyError::BlockedHost(_)));
        // Numeric aliases that the URL parser normalizes to 127.0.0.1.
        assert_eq!(err("http://2130706433:8765/hypernavigator.json"), PolicyError::NonCanonicalOrigin);
        assert_eq!(err("http://127.1:8765/hypernavigator.json"), PolicyError::NonCanonicalOrigin);
        assert_eq!(err("http://0x7f.0.0.1:8765/hypernavigator.json"), PolicyError::NonCanonicalOrigin);
        assert_eq!(err("HTTP://127.0.0.1:8765/hypernavigator.json"), PolicyError::NonCanonicalOrigin);
    }

    #[test]
    fn rejects_unexpected_ports() {
        assert_eq!(err("http://127.0.0.1/hypernavigator.json"), PolicyError::BlockedPort(None));
        assert_eq!(err("http://127.0.0.1:80/hypernavigator.json"), PolicyError::BlockedPort(None));
        assert_eq!(err("http://127.0.0.1:8766/hypernavigator.json"), PolicyError::BlockedPort(Some(8766)));
        assert_eq!(err("http://127.0.0.1:08765/hypernavigator.json"), PolicyError::NonCanonicalOrigin);
    }

    #[test]
    fn rejects_userinfo() {
        assert_eq!(err("http://user@127.0.0.1:8765/hypernavigator.json"), PolicyError::UserInfo);
        assert_eq!(err("http://user:pw@127.0.0.1:8765/hypernavigator.json"), PolicyError::UserInfo);
        assert!(matches!(err("http://127.0.0.1:8765@evil.example/hypernavigator.json"), PolicyError::UserInfo));
    }

    #[test]
    fn rejects_traversal_and_encoding() {
        assert_eq!(err("http://127.0.0.1:8765/a/../hypernavigator.json"), PolicyError::Traversal);
        assert_eq!(err("http://127.0.0.1:8765/a/./hypernavigator.json"), PolicyError::Traversal);
        assert_eq!(err("http://127.0.0.1:8765//hypernavigator.json"), PolicyError::Traversal);
        assert_eq!(err("http://127.0.0.1:8765/a/%2e%2e/hypernavigator.json"), PolicyError::ForbiddenPathCharacters);
        assert_eq!(err("http://127.0.0.1:8765/a%2fb/hypernavigator.json"), PolicyError::ForbiddenPathCharacters);
        assert_eq!(err("http://127.0.0.1:8765/a\\..\\hypernavigator.json"), PolicyError::InvalidCharacters);
        assert_eq!(err("http://127.0.0.1:8765/a b/hypernavigator.json"), PolicyError::InvalidCharacters);
        assert_eq!(err("http://127.0.0.1:8765/a/hypernavigator.json?x=javascript:1"), PolicyError::QueryOrFragment);
        assert_eq!(err("http://127.0.0.1:8765/a/hypernavigator.json#frag"), PolicyError::QueryOrFragment);
    }

    #[test]
    fn rejects_non_manifest_paths_and_bad_input() {
        assert_eq!(err("http://127.0.0.1:8765/counter/index.html"), PolicyError::NotAManifestPath);
        assert_eq!(err(""), PolicyError::Empty);
        assert_eq!(err("not a url"), PolicyError::InvalidCharacters);
        assert_eq!(err("127.0.0.1:8765/hypernavigator.json"), PolicyError::Malformed);
        assert_eq!(err(&format!("http://127.0.0.1:8765/{}", "a/".repeat(1100))), PolicyError::TooLong);
    }

    fn manifest_url() -> Url {
        Url::parse("http://127.0.0.1:8765/counter/hypernavigator.json").unwrap()
    }

    #[test]
    fn resolves_relative_entries() {
        let m = manifest_url();
        assert_eq!(P.resolve_entry(&m, "index.html").unwrap().as_str(), "http://127.0.0.1:8765/counter/index.html");
        assert_eq!(P.resolve_entry(&m, "app/main.html").unwrap().as_str(), "http://127.0.0.1:8765/counter/app/main.html");
    }

    #[test]
    fn rejects_escaping_or_absolute_entries() {
        let m = manifest_url();
        for bad in [
            "",
            "/index.html",
            "../index.html",
            "a/../../index.html",
            "./index.html",
            "a//index.html",
            "http://evil.example/index.html",
            "//evil.example/index.html",
            "javascript:alert(1)",
            "data:text/html,x",
            "%2e%2e/index.html",
            "index.html?x=1",
            "index.html#x",
            "..\\index.html",
            "index.js",
            "index.html ",
        ] {
            assert!(P.resolve_entry(&m, bad).is_err(), "entry {bad:?} should be rejected");
        }
        assert!(P.resolve_entry(&m, &format!("{}.html", "a".repeat(300))).is_err());
    }

    #[test]
    fn guest_navigation_is_limited_to_the_dev_origin() {
        for allowed in ["http://127.0.0.1:8765/counter/index.html", "http://127.0.0.1:8765/other/page.html?q=1#x"] {
            assert!(P.allows_guest_navigation(&Url::parse(allowed).unwrap()), "{allowed}");
        }
        for blocked in [
            "https://example.com/",
            "http://127.0.0.1:8766/",
            "http://localhost:8765/",
            "http://[::1]:8765/",
            "http://user@127.0.0.1:8765/",
            "file:///etc/passwd",
            "about:blank",
            "data:text/html,x",
            "tauri://localhost/index.html",
            "blob:http://127.0.0.1:8765/1234",
        ] {
            assert!(!P.allows_guest_navigation(&Url::parse(blocked).unwrap()), "{blocked}");
        }
    }
}
