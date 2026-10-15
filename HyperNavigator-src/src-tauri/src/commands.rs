//! Commands callable only by the trusted chrome WebView.
//!
//! Each command name is listed in `build.rs` (`AppManifest::commands`) so Tauri generates an
//! `allow-<command>` permission for it, and `capabilities/chrome.json` grants those permissions
//! to the `chrome` WebView label only. Lock order is always `nav` then `share`; no lock is held
//! while touching a WebView (the guest's navigation handler takes the same locks).

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State, Window};

use crate::guest::{self, GuestTarget};
use crate::manifest::fetch_app;
use crate::navigator::{GuestContent, NavAction, NavState, SharedFolderInfo};
use crate::permissions::{self, ActiveGrant, MAX_SHARED_ENTRIES};
use crate::policy::OriginPolicy;
use crate::{layout, HostState};

fn snapshot(state: &HostState) -> NavState {
    let nav = state.nav.lock().unwrap();
    let share = state.share.lock().unwrap();
    let shared = share
        .grant()
        .filter(|g| g.guest_session == nav.session())
        .map(|g| SharedFolderInfo { folder_name: g.folder_name.clone(), entries: g.shared_entries });
    NavState::from_navigator(&nav, shared)
}

#[tauri::command]
pub fn nav_state(state: State<'_, HostState>) -> NavState {
    snapshot(&state)
}

/// Go: validates the address-bar input, then loads its manifest.
#[tauri::command]
pub async fn navigate<R: Runtime>(app: AppHandle<R>, url: String) -> Result<NavState, String> {
    let manifest_url = OriginPolicy::DEV.parse_manifest_url(&url).map_err(|e| e.to_string())?;
    run_navigation(&app, NavAction::Go, Some(manifest_url)).await
}

#[tauri::command]
pub async fn go_back<R: Runtime>(app: AppHandle<R>) -> Result<NavState, String> {
    run_navigation(&app, NavAction::Back, None).await
}

#[tauri::command]
pub async fn go_forward<R: Runtime>(app: AppHandle<R>) -> Result<NavState, String> {
    run_navigation(&app, NavAction::Forward, None).await
}

/// Re-fetches the manifest and loads the app into a fresh guest WebView (empty cache).
#[tauri::command]
pub async fn reload<R: Runtime>(app: AppHandle<R>) -> Result<NavState, String> {
    run_navigation(&app, NavAction::Reload, None).await
}

async fn run_navigation<R: Runtime>(app: &AppHandle<R>, action: NavAction, url: Option<url::Url>) -> Result<NavState, String> {
    let state = app.state::<HostState>();
    let pending = state.nav.lock().unwrap().begin(action, url).map_err(|e| e.to_string())?;

    let fetch_url = pending.url.clone();
    let result = tauri::async_runtime::spawn_blocking(move || fetch_app(&OriginPolicy::DEV, &fetch_url))
        .await
        .map_err(|e| e.to_string())?;
    let (content, target) = match result {
        Ok(resolved) => {
            let entry_url = resolved.entry_url.clone();
            (GuestContent::App(Box::new(resolved)), GuestTarget::App { entry_url })
        }
        Err(e) => {
            let message = e.to_string();
            (GuestContent::Error(message.clone()), GuestTarget::Placeholder { state: "error", message: Some(message) })
        }
    };

    let session = {
        let mut nav = state.nav.lock().unwrap();
        let session = nav.commit(&pending, content);
        if session.is_some() {
            state.share.lock().unwrap().clear();
        }
        session
    };
    if let Some(session) = session {
        guest::replace_guest(app, session, target).map_err(|e| e.to_string())?;
    }
    Ok(snapshot(&state))
}

/// What the trusted confirmation dialog shows.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharePrompt {
    pub request_id: u64,
    pub folder_name: String,
    pub app_name: String,
    pub app_origin: String,
    pub max_entries: usize,
}

fn set_overlay<R: Runtime>(app: &AppHandle<R>, on: bool) -> Result<(), String> {
    let state = app.state::<HostState>();
    state.overlay.store(on, std::sync::atomic::Ordering::SeqCst);
    let guest_label = state.guest_label.lock().unwrap().clone();
    let window = app.get_window(crate::WINDOW_LABEL).ok_or("main window missing")?;
    layout::apply(&window, guest_label.as_deref(), on).map_err(|e| e.to_string())
}

/// Opens the native folder picker. Cancelling returns `None` and changes nothing. On a pick, the
/// chrome expands over the (hidden) guest to show the confirmation.
#[tauri::command]
pub async fn pick_share_folder<R: Runtime>(app: AppHandle<R>, window: Window<R>) -> Result<Option<SharePrompt>, String> {
    let state = app.state::<HostState>();
    let (session, app_name, app_origin) = {
        let nav = state.nav.lock().unwrap();
        match nav.content() {
            GuestContent::App(a) => (nav.session(), a.manifest.name.clone(), a.entry_url.origin().ascii_serialization()),
            _ => return Err("Load an app first; folder names can only be shared with a running app".into()),
        }
    };

    let picked = rfd::AsyncFileDialog::new()
        .set_title("Choose a folder whose item names to share")
        .set_parent(&window)
        .pick_folder()
        .await;
    let Some(handle) = picked else { return Ok(None) };
    let path = handle.path().to_path_buf();
    let folder_name = permissions::folder_display_name(&path);

    let request_id = {
        let nav = state.nav.lock().unwrap();
        if nav.session() != session {
            return Err("The app changed while the folder picker was open; nothing was shared".into());
        }
        state.share.lock().unwrap().begin(session, path)
    };
    set_overlay(&app, true)?;
    Ok(Some(SharePrompt { request_id, folder_name, app_name, app_origin, max_entries: MAX_SHARED_ENTRIES }))
}

/// Applies the user's decision from the confirmation dialog and restores the normal layout.
#[tauri::command]
pub async fn resolve_share_folder<R: Runtime>(app: AppHandle<R>, request_id: u64, approve: bool) -> Result<NavState, String> {
    set_overlay(&app, false)?;
    let state = app.state::<HostState>();
    if !approve {
        state.share.lock().unwrap().cancel();
        return Ok(snapshot(&state));
    }

    let (session, entry_url) = {
        let nav = state.nav.lock().unwrap();
        match nav.content() {
            GuestContent::App(a) => (nav.session(), a.entry_url.clone()),
            _ => (nav.session(), url::Url::parse("about:blank").unwrap()),
        }
    };
    let pending = state
        .share
        .lock()
        .unwrap()
        .take_pending(request_id, session)
        .ok_or("This request expired because the app navigated or reloaded; nothing was shared")?;
    if entry_url.scheme() == "about" {
        return Err("No app is loaded; nothing was shared".into());
    }

    let path = pending.path.clone();
    let listing = tauri::async_runtime::spawn_blocking(move || permissions::list_folder(&path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let script = permissions::guest_delivery_script(&listing, &entry_url.origin().ascii_serialization(), entry_url.path());

    let guest = app.get_webview(&guest::guest_label(session)).ok_or("The app's view is no longer available")?;
    if state.nav.lock().unwrap().session() != session {
        return Err("The app navigated before the names were sent; nothing was shared".into());
    }
    guest.eval(script).map_err(|e| e.to_string())?;

    {
        let nav = state.nav.lock().unwrap();
        if nav.session() == session {
            state.share.lock().unwrap().set_grant(ActiveGrant {
                guest_session: session,
                folder_name: permissions::folder_display_name(&pending.path),
                shared_entries: listing.entries.len(),
            });
        }
    }
    Ok(snapshot(&state))
}
