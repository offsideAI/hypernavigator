//! Read-only folder-name sharing: the only capability the guest can receive in v0.1.
//!
//! Rust enumerates at most [`MAX_SHARED_ENTRIES`] immediate children of a user-picked folder,
//! never follows symlinks, never recurses, never reads file contents, and never exposes paths.
//! The result reaches the guest as a JSON string inside a `CustomEvent`, built by
//! [`guest_delivery_script`] without interpolating raw filenames into JavaScript source.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub const MAX_SHARED_ENTRIES: usize = 100;
/// Upper bound on directory entries read before sorting, so huge folders stay cheap.
pub const MAX_SCANNED_ENTRIES: usize = 10_000;
pub const MAX_NAME_CHARS: usize = 255;
pub const FOLDER_LISTING_EVENT: &str = "hypernavigator:folder-listing";
pub const FOLDER_LISTING_TYPE: &str = "hypernavigator.folderListing";
pub const FOLDER_LISTING_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SharedEntry {
    pub name: String,
    pub kind: EntryKind,
}

/// Versioned payload sent to the guest. Contains no paths, sizes, timestamps or contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderListing {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub version: u32,
    pub truncated: bool,
    pub entries: Vec<SharedEntry>,
}

#[derive(Debug)]
pub enum ListingError {
    NotADirectory,
    SymlinkRoot,
    Io(io::Error),
}

impl std::fmt::Display for ListingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotADirectory => write!(f, "The selected item is not a folder"),
            Self::SymlinkRoot => write!(f, "The selected folder is a symbolic link; choose the real folder"),
            Self::Io(e) => write!(f, "Could not read the folder: {e}"),
        }
    }
}

impl std::error::Error for ListingError {}

/// Display-safe name: invalid UTF-8 and control / bidi-override characters become U+FFFD,
/// then the name is capped at [`MAX_NAME_CHARS`] characters.
pub fn sanitize_name(raw: &std::ffi::OsStr) -> String {
    raw.to_string_lossy()
        .chars()
        .map(|c| if c.is_control() || is_bidi_control(c) { '\u{FFFD}' } else { c })
        .take(MAX_NAME_CHARS)
        .collect()
}

fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Lists immediate children of `dir`. Symlinks are reported as `symlink` and never dereferenced.
pub fn list_folder(dir: &Path) -> Result<FolderListing, ListingError> {
    let meta = fs::symlink_metadata(dir).map_err(ListingError::Io)?;
    if meta.file_type().is_symlink() {
        return Err(ListingError::SymlinkRoot);
    }
    if !meta.is_dir() {
        return Err(ListingError::NotADirectory);
    }

    let mut entries = Vec::new();
    let mut more_than_scanned = false;
    for (i, entry) in fs::read_dir(dir).map_err(ListingError::Io)?.enumerate() {
        if i >= MAX_SCANNED_ENTRIES {
            more_than_scanned = true;
            break;
        }
        let Ok(entry) = entry else { continue };
        // DirEntry::file_type does not follow symlinks.
        let kind = match entry.file_type() {
            Ok(t) if t.is_symlink() => EntryKind::Symlink,
            Ok(t) if t.is_dir() => EntryKind::Directory,
            Ok(t) if t.is_file() => EntryKind::File,
            _ => EntryKind::Other,
        };
        entries.push(SharedEntry { name: sanitize_name(&entry.file_name()), kind });
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name).then(a.kind.cmp(&b.kind)));
    let truncated = more_than_scanned || entries.len() > MAX_SHARED_ENTRIES;
    entries.truncate(MAX_SHARED_ENTRIES);

    Ok(FolderListing { message_type: FOLDER_LISTING_TYPE, version: FOLDER_LISTING_VERSION, truncated, entries })
}

/// The last path component, for the trusted chrome's confirmation text only.
pub fn folder_display_name(path: &Path) -> String {
    path.file_name().map(sanitize_name).unwrap_or_else(|| "/".to_string())
}

/// Encodes `value` as a JavaScript string literal. JSON string syntax is valid JS; U+2028/U+2029 are
/// additionally escaped for older parsers.
fn js_string_literal(value: &str) -> String {
    serde_json::to_string(value)
        .expect("strings always serialize")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// Builds the script the Host evaluates in the guest. Filenames only ever appear inside a
/// JSON-encoded string literal that the guest parses with `JSON.parse`; the script also checks it
/// is still running on the expected document before dispatching.
pub fn guest_delivery_script(listing: &FolderListing, expected_origin: &str, expected_path: &str) -> String {
    let payload = serde_json::to_string(listing).expect("listing serializes");
    format!(
        "(function(){{\
if(window.location.origin!=={origin}||window.location.pathname!=={path})return;\
window.dispatchEvent(new CustomEvent({event},{{detail:{payload}}}));\
}})();",
        origin = js_string_literal(expected_origin),
        path = js_string_literal(expected_path),
        event = js_string_literal(FOLDER_LISTING_EVENT),
        payload = js_string_literal(&payload),
    )
}

/// A folder the user picked but has not yet approved.
#[derive(Debug, Clone)]
pub struct PendingShare {
    pub request_id: u64,
    pub guest_session: u64,
    pub path: PathBuf,
}

/// An approved, session-scoped grant. Dropped whenever the guest session changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveGrant {
    pub guest_session: u64,
    pub folder_name: String,
    pub shared_entries: usize,
}

#[derive(Debug, Default)]
pub struct ShareState {
    next_request_id: u64,
    pending: Option<PendingShare>,
    grant: Option<ActiveGrant>,
}

impl ShareState {
    pub fn begin(&mut self, guest_session: u64, path: PathBuf) -> u64 {
        self.next_request_id += 1;
        self.pending = Some(PendingShare { request_id: self.next_request_id, guest_session, path });
        self.next_request_id
    }

    /// Takes the pending request only if both the id and the guest session still match.
    pub fn take_pending(&mut self, request_id: u64, current_session: u64) -> Option<PendingShare> {
        match &self.pending {
            Some(p) if p.request_id == request_id && p.guest_session == current_session => self.pending.take(),
            _ => {
                self.pending = None;
                None
            }
        }
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }

    pub fn set_grant(&mut self, grant: ActiveGrant) {
        self.grant = Some(grant);
    }

    pub fn grant(&self) -> Option<&ActiveGrant> {
        self.grant.as_ref()
    }

    /// Called on every guest navigation / new session.
    pub fn clear(&mut self) {
        self.pending = None;
        self.grant = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    #[test]
    fn lists_names_and_types_sorted_without_paths() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.txt"), b"secret contents").unwrap();
        fs::create_dir(dir.path().join("a-dir")).unwrap();
        fs::write(dir.path().join("a-dir").join("nested.txt"), b"x").unwrap();
        fs::write(dir.path().join("C.md"), b"").unwrap();

        let listing = list_folder(dir.path()).unwrap();
        let names: Vec<_> = listing.entries.iter().map(|e| (e.name.as_str(), e.kind)).collect();
        assert_eq!(names, vec![("C.md", EntryKind::File), ("a-dir", EntryKind::Directory), ("b.txt", EntryKind::File)]);
        assert!(!listing.truncated);

        let json = serde_json::to_string(&listing).unwrap();
        assert!(!json.contains(dir.path().to_str().unwrap()), "absolute path leaked: {json}");
        assert!(!json.contains("secret contents"));
        assert!(!json.contains("nested.txt"), "listing must not recurse");
    }

    #[test]
    fn does_not_follow_symlinks() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("outside-secret.txt"), b"x").unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link-to-dir")).unwrap();
        std::os::unix::fs::symlink("/nonexistent/target", dir.path().join("dangling")).unwrap();

        let listing = list_folder(dir.path()).unwrap();
        assert_eq!(
            listing.entries,
            vec![
                SharedEntry { name: "dangling".into(), kind: EntryKind::Symlink },
                SharedEntry { name: "link-to-dir".into(), kind: EntryKind::Symlink },
            ]
        );
        assert!(!serde_json::to_string(&listing).unwrap().contains("outside-secret"));
    }

    #[test]
    fn rejects_symlinked_root_and_files() {
        let real = tempfile::tempdir().unwrap();
        let holder = tempfile::tempdir().unwrap();
        let link = holder.path().join("link");
        std::os::unix::fs::symlink(real.path(), &link).unwrap();
        assert!(matches!(list_folder(&link), Err(ListingError::SymlinkRoot)));

        let file = holder.path().join("file.txt");
        fs::write(&file, b"x").unwrap();
        assert!(matches!(list_folder(&file), Err(ListingError::NotADirectory)));
        assert!(matches!(list_folder(&holder.path().join("missing")), Err(ListingError::Io(_))));
    }

    #[test]
    fn caps_at_100_entries_deterministically() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..150 {
            fs::write(dir.path().join(format!("f{i:03}")), b"").unwrap();
        }
        let first = list_folder(dir.path()).unwrap();
        assert_eq!(first.entries.len(), MAX_SHARED_ENTRIES);
        assert!(first.truncated);
        assert_eq!(first.entries[0].name, "f000");
        assert_eq!(first.entries[99].name, "f099");
        assert_eq!(list_folder(dir.path()).unwrap(), first);
    }

    #[test]
    fn sanitizes_hostile_names() {
        assert_eq!(sanitize_name(OsStr::new("ok name.txt")), "ok name.txt");
        assert_eq!(sanitize_name(OsStr::new("line\nbreak\t")), "line\u{FFFD}break\u{FFFD}");
        assert_eq!(sanitize_name(OsStr::new("evil\u{202E}txt.exe")), "evil\u{FFFD}txt.exe");
        assert_eq!(sanitize_name(OsStr::from_bytes(b"bad\xffutf8")), "bad\u{FFFD}utf8");
        assert_eq!(sanitize_name(OsStr::new(&"x".repeat(400))).chars().count(), MAX_NAME_CHARS);
    }

    fn listing_named(name: &str) -> FolderListing {
        FolderListing {
            message_type: FOLDER_LISTING_TYPE,
            version: 1,
            truncated: false,
            entries: vec![SharedEntry { name: name.into(), kind: EntryKind::File }],
        }
    }

    /// Splits a delivery script into (code before the detail literal, the literal, code after).
    fn split_detail(script: &str) -> (&str, &str, &str) {
        let start = script.find("detail:").unwrap() + "detail:".len();
        let end = script.rfind("}));").unwrap();
        (&script[..start], &script[start..end], &script[end..])
    }

    #[test]
    fn delivery_script_never_contains_raw_filenames() {
        let hostile = "\");alert(document.cookie);(\"</script>\u{2028}`${x}`\\";
        let benign = guest_delivery_script(&listing_named("notes.txt"), "http://127.0.0.1:8765", "/counter/index.html");
        let script = guest_delivery_script(&listing_named(hostile), "http://127.0.0.1:8765", "/counter/index.html");
        assert!(!script.contains(hostile));
        assert!(!script.contains('\u{2028}'));

        // Filenames can only influence the string literal: the executable code around it is identical.
        let (before, literal, after) = split_detail(&script);
        let (benign_before, _, benign_after) = split_detail(&benign);
        assert_eq!((before, after), (benign_before, benign_after));

        // The literal is one JSON/JS string that decodes back to exactly the serialized payload.
        let decoded: String = serde_json::from_str(literal).unwrap();
        let roundtrip: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(roundtrip["entries"][0]["name"], hostile);
        assert_eq!(roundtrip["type"], FOLDER_LISTING_TYPE);
        assert_eq!(roundtrip["version"], 1);
    }

    #[test]
    fn share_state_is_bound_to_request_and_session() {
        let mut s = ShareState::default();
        let id = s.begin(7, PathBuf::from("/tmp"));
        assert!(s.take_pending(id + 1, 7).is_none(), "wrong request id");
        let id = s.begin(7, PathBuf::from("/tmp"));
        assert!(s.take_pending(id, 8).is_none(), "guest navigated since the prompt");
        let id = s.begin(7, PathBuf::from("/tmp"));
        assert!(s.take_pending(id, 7).is_some());
        assert!(s.take_pending(id, 7).is_none(), "single use");

        s.set_grant(ActiveGrant { guest_session: 7, folder_name: "x".into(), shared_entries: 1 });
        s.begin(7, PathBuf::from("/tmp"));
        s.clear();
        assert!(s.grant().is_none());
        assert!(s.take_pending(id + 1, 7).is_none());
    }
}
