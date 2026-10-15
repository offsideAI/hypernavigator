//! Guest WebView lifecycle.
//!
//! Every Host-initiated load (Go, Back, Forward, Reload, error page) creates a brand-new child
//! WebView labelled `guest-<session>` with a non-persistent (incognito) data store, then closes the
//! previous one. A fresh WebView means a fresh HTTP cache (Reload always picks up a rebuilt app),
//! no storage carried between app sessions, and no Host state tied to an old document.
//!
//! No capability targets `guest-*`, so the guest gets no Tauri command, event, or plugin access.

use serde::Serialize;
use tauri::webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::window::Color;
use tauri::{AppHandle, Emitter, EventTarget, Manager, Runtime, Url, WebviewUrl};

use crate::layout;
use crate::policy::OriginPolicy;
use crate::HostState;

pub const GUEST_LABEL_PREFIX: &str = "guest-";

/// Fixes `MouseEvent.buttons` for macOS trackpad tap-to-click so Compose sees the press
/// (see the comment in the file). Static, Host-owned script; it carries no data.
pub const INPUT_SHIM: &str = include_str!("guest_input_shim.js");
pub const PLACEHOLDER_PAGE: &str = "placeholder.html";
pub const GUEST_EVENT: &str = "hn:guest";

#[derive(Debug, Clone)]
pub enum GuestTarget {
    /// Local "ready" / error page bundled with the Host.
    Placeholder { state: &'static str, message: Option<String> },
    /// The validated app entry on the dev origin.
    App { entry_url: Url },
}

/// Guest lifecycle notifications for the trusted chrome.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum GuestEvent {
    PageLoad { session: u64, finished: bool },
    Blocked { session: u64, url: String },
    LeftApp { session: u64, url: String },
}

pub fn guest_label(session: u64) -> String {
    format!("{GUEST_LABEL_PREFIX}{session}")
}

fn emit_to_chrome<R: Runtime>(app: &AppHandle<R>, event: GuestEvent) {
    let _ = app.emit_to(EventTarget::webview(crate::CHROME_LABEL), GUEST_EVENT, event);
}

fn display_url(url: &Url) -> String {
    url.as_str().chars().take(200).collect()
}

fn without_fragment(url: &Url) -> Url {
    let mut u = url.clone();
    u.set_fragment(None);
    u
}

/// The chrome's own origin (`http://localhost:1420` in dev, `tauri://localhost` when bundled).
fn app_base_url<R: Runtime>(app: &AppHandle<R>) -> Option<Url> {
    app.get_webview(crate::CHROME_LABEL).and_then(|w| w.url().ok())
}

/// Replaces the guest WebView for `session`.
pub fn replace_guest<R: Runtime>(app: &AppHandle<R>, session: u64, target: GuestTarget) -> tauri::Result<()> {
    let window = app.get_window(crate::WINDOW_LABEL).ok_or(tauri::Error::WindowNotFound)?;
    let state = app.state::<HostState>();
    let label = guest_label(session);

    let placeholder_url = app_base_url(app).and_then(|base| base.join(PLACEHOLDER_PAGE).ok());
    let (webview_url, allowed_entry) = match &target {
        GuestTarget::Placeholder { state: page_state, message } => {
            let mut fragment = format!("state={page_state}");
            if let Some(message) = message {
                let encoded: String = url::form_urlencoded::byte_serialize(message.as_bytes()).collect();
                fragment.push_str("&message=");
                fragment.push_str(&encoded);
            }
            (WebviewUrl::App(format!("{PLACEHOLDER_PAGE}#{fragment}").into()), None)
        }
        GuestTarget::App { entry_url } => (WebviewUrl::External(entry_url.clone()), Some(entry_url.clone())),
    };

    let nav_app = app.clone();
    let load_app = app.clone();
    let title_app = app.clone();
    let mut builder = WebviewBuilder::new(&label, webview_url)
        .incognito(true)
        .initialization_script(INPUT_SHIM)
        .background_color(Color(17, 17, 19, 255))
        .on_navigation(move |url| {
            let policy = OriginPolicy::DEV;
            let allowed = match &allowed_entry {
                Some(entry) => {
                    let ok = policy.allows_guest_navigation(url);
                    if ok && without_fragment(url) != without_fragment(entry) {
                        // Same origin, but not the document HyperNavigator loaded: end the session's grant.
                        let state = nav_app.state::<HostState>();
                        let mut nav = state.nav.lock().unwrap();
                        if nav.session() == session {
                            nav.guest_navigated_itself();
                            state.share.lock().unwrap().clear();
                            emit_to_chrome(&nav_app, GuestEvent::LeftApp { session, url: display_url(url) });
                        }
                    }
                    ok
                }
                None => placeholder_url.as_ref().is_some_and(|p| without_fragment(url) == *p),
            };
            if !allowed {
                if cfg!(debug_assertions) {
                    eprintln!("[hn] blocked guest navigation (session {session}): {}", display_url(url));
                }
                emit_to_chrome(&nav_app, GuestEvent::Blocked { session, url: display_url(url) });
            }
            allowed
        })
        .on_new_window(|_url, _features| NewWindowResponse::Deny)
        .on_download(|_webview, _event| false)
        .on_page_load(move |webview, payload| {
            let finished = matches!(payload.event(), PageLoadEvent::Finished);
            emit_to_chrome(&load_app, GuestEvent::PageLoad { session, finished });
            if finished && OriginPolicy::DEV.allows_guest_navigation(payload.url()) {
                crate::security_probe::run(&webview);
            }
        });
    builder = crate::security_probe::attach(builder, title_app);

    let (width, height) = layout::logical_inner_size(&window)?;
    let overlay = state.overlay.load(std::sync::atomic::Ordering::SeqCst);
    let (_, bounds) = layout::compute(width, height, overlay);
    let rect = bounds.to_rect();
    let guest = window.add_child(builder, rect.position, rect.size)?;
    if overlay {
        guest.hide()?;
    }

    let previous = state.guest_label.lock().unwrap().replace(label);
    if let Some(previous) = previous.and_then(|l| app.get_webview(&l)) {
        previous.close()?;
    }
    Ok(())
}
