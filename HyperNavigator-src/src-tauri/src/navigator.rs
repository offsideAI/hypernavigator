//! Manifest-level navigation history.
//!
//! History entries are manifest URLs, not raw WebView history: Back/Forward/Reload re-fetch and
//! re-validate the manifest and load the app into a fresh guest WebView. Navigations the guest
//! performs by itself are not recorded. Every committed navigation (success or error page) gets a
//! new guest session id; grants are bound to that id.

use serde::Serialize;
use url::Url;

use crate::manifest::ResolvedApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavAction {
    Go,
    Back,
    Forward,
    Reload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavError {
    NoBackHistory,
    NoForwardHistory,
    NothingToReload,
}

impl std::fmt::Display for NavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBackHistory => write!(f, "There is no earlier app in this window's history"),
            Self::NoForwardHistory => write!(f, "There is no later app in this window's history"),
            Self::NothingToReload => write!(f, "Nothing to reload yet - enter a manifest URL and press Go"),
        }
    }
}

/// A navigation that has been started but not yet committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingNav {
    pub seq: u64,
    pub action: NavAction,
    pub url: Url,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestContent {
    Ready,
    App(Box<ResolvedApp>),
    Error(String),
}

#[derive(Debug)]
pub struct Navigator {
    history: Vec<Url>,
    index: Option<usize>,
    latest_seq: u64,
    session: u64,
    content: GuestContent,
}

impl Default for Navigator {
    fn default() -> Self {
        Self { history: Vec::new(), index: None, latest_seq: 0, session: 0, content: GuestContent::Ready }
    }
}

impl Navigator {
    pub fn session(&self) -> u64 {
        self.session
    }

    pub fn content(&self) -> &GuestContent {
        &self.content
    }

    pub fn current_url(&self) -> Option<&Url> {
        self.index.map(|i| &self.history[i])
    }

    pub fn can_go_back(&self) -> bool {
        matches!(self.index, Some(i) if i > 0)
    }

    pub fn can_go_forward(&self) -> bool {
        matches!(self.index, Some(i) if i + 1 < self.history.len())
    }

    /// Starts a navigation. `go_url` is required for [`NavAction::Go`] and ignored otherwise.
    pub fn begin(&mut self, action: NavAction, go_url: Option<Url>) -> Result<PendingNav, NavError> {
        let url = match action {
            NavAction::Go => go_url.expect("Go requires a URL"),
            NavAction::Back => {
                let i = self.index.filter(|i| *i > 0).ok_or(NavError::NoBackHistory)?;
                self.history[i - 1].clone()
            }
            NavAction::Forward => {
                let i = self.index.filter(|i| i + 1 < self.history.len()).ok_or(NavError::NoForwardHistory)?;
                self.history[i + 1].clone()
            }
            NavAction::Reload => self.current_url().cloned().ok_or(NavError::NothingToReload)?,
        };
        self.latest_seq += 1;
        Ok(PendingNav { seq: self.latest_seq, action, url })
    }

    /// Records the outcome. Returns the new guest session id, or `None` if a newer navigation
    /// started meanwhile (the stale result must be discarded).
    pub fn commit(&mut self, pending: &PendingNav, content: GuestContent) -> Option<u64> {
        if pending.seq != self.latest_seq {
            return None;
        }
        match pending.action {
            NavAction::Go => {
                let next = self.index.map_or(0, |i| i + 1);
                self.history.truncate(next);
                self.history.push(pending.url.clone());
                self.index = Some(next);
            }
            NavAction::Back => self.index = self.index.map(|i| i - 1),
            NavAction::Forward => self.index = self.index.map(|i| i + 1),
            NavAction::Reload => {}
        }
        self.content = content;
        Some(self.new_session())
    }

    /// The guest left the Host-loaded document by itself; drop anything bound to the old session.
    pub fn guest_navigated_itself(&mut self) -> u64 {
        self.new_session()
    }

    fn new_session(&mut self) -> u64 {
        self.session += 1;
        self.session
    }
}

/// Snapshot sent to the trusted chrome.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavState {
    pub url: Option<String>,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub can_reload: bool,
    pub status: &'static str,
    pub message: Option<String>,
    pub app: Option<AppInfo>,
    pub shared_folder: Option<SharedFolderInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub origin: String,
    pub entry_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedFolderInfo {
    pub folder_name: String,
    pub entries: usize,
}

impl NavState {
    pub fn from_navigator(nav: &Navigator, shared_folder: Option<SharedFolderInfo>) -> Self {
        let (status, message, app) = match nav.content() {
            GuestContent::Ready => ("ready", None, None),
            GuestContent::Error(e) => ("error", Some(e.clone()), None),
            GuestContent::App(app) => (
                "loaded",
                None,
                Some(AppInfo {
                    id: app.manifest.id.clone(),
                    name: app.manifest.name.clone(),
                    version: app.manifest.version.clone(),
                    origin: app.entry_url.origin().ascii_serialization(),
                    entry_url: app.entry_url.to_string(),
                }),
            ),
        };
        Self {
            url: nav.current_url().map(Url::to_string),
            can_go_back: nav.can_go_back(),
            can_go_forward: nav.can_go_forward(),
            can_reload: nav.current_url().is_some(),
            status,
            message,
            app,
            shared_folder,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(path: &str) -> Url {
        Url::parse(&format!("http://127.0.0.1:8765/{path}/hypernavigator.json")).unwrap()
    }

    fn go(nav: &mut Navigator, path: &str) -> u64 {
        let p = nav.begin(NavAction::Go, Some(url(path))).unwrap();
        nav.commit(&p, GuestContent::Error("x".into())).unwrap()
    }

    #[test]
    fn empty_history_has_nothing_to_do() {
        let mut nav = Navigator::default();
        assert_eq!(nav.begin(NavAction::Back, None), Err(NavError::NoBackHistory));
        assert_eq!(nav.begin(NavAction::Forward, None), Err(NavError::NoForwardHistory));
        assert_eq!(nav.begin(NavAction::Reload, None), Err(NavError::NothingToReload));
        assert_eq!(nav.content(), &GuestContent::Ready);
    }

    #[test]
    fn back_forward_reload_walk_manifest_history() {
        let mut nav = Navigator::default();
        go(&mut nav, "a");
        go(&mut nav, "b");
        assert!(nav.can_go_back() && !nav.can_go_forward());

        let p = nav.begin(NavAction::Back, None).unwrap();
        assert_eq!(p.url, url("a"));
        nav.commit(&p, GuestContent::Ready).unwrap();
        assert!(!nav.can_go_back() && nav.can_go_forward());

        let p = nav.begin(NavAction::Reload, None).unwrap();
        assert_eq!(p.url, url("a"));
        nav.commit(&p, GuestContent::Ready).unwrap();
        assert_eq!(nav.current_url(), Some(&url("a")));

        let p = nav.begin(NavAction::Forward, None).unwrap();
        assert_eq!(p.url, url("b"));
        nav.commit(&p, GuestContent::Ready).unwrap();
        assert_eq!(nav.current_url(), Some(&url("b")));
    }

    #[test]
    fn go_after_back_drops_forward_entries() {
        let mut nav = Navigator::default();
        go(&mut nav, "a");
        go(&mut nav, "b");
        let p = nav.begin(NavAction::Back, None).unwrap();
        nav.commit(&p, GuestContent::Ready);
        go(&mut nav, "c");
        assert!(!nav.can_go_forward());
        let p = nav.begin(NavAction::Back, None).unwrap();
        assert_eq!(p.url, url("a"));
    }

    #[test]
    fn stale_results_are_discarded_and_sessions_advance() {
        let mut nav = Navigator::default();
        let slow = nav.begin(NavAction::Go, Some(url("slow"))).unwrap();
        let fast = nav.begin(NavAction::Go, Some(url("fast"))).unwrap();
        let s1 = nav.commit(&fast, GuestContent::Ready).unwrap();
        assert_eq!(nav.commit(&slow, GuestContent::Ready), None);
        assert_eq!(nav.current_url(), Some(&url("fast")));
        let s2 = nav.guest_navigated_itself();
        assert!(s2 > s1);
        assert_eq!(nav.session(), s2);
    }
}
