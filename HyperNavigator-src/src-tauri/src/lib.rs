//! HyperNavigator Host (developer preview).
//!
//! One native window holds two child WebViews: the trusted `chrome` (bundled TypeScript UI with
//! narrowly scoped commands) and a `guest-<n>` WebView that loads a Kotlin/Wasm Compose app from
//! the allowlisted dev origin with no Tauri privileges. Trusted localhost development only.

pub mod commands;
pub mod guest;
pub mod layout;
pub mod manifest;
pub mod navigator;
pub mod permissions;
pub mod policy;
pub mod security_probe;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use tauri::webview::{NewWindowResponse, WebviewBuilder};
use tauri::window::Color;
use tauri::{AppHandle, Manager, Runtime, Theme, TitleBarStyle, Url, WebviewUrl, WindowEvent};

use crate::guest::GuestTarget;
use crate::navigator::Navigator;
use crate::permissions::ShareState;

pub const WINDOW_LABEL: &str = "main";
pub const CHROME_LABEL: &str = "chrome";

/// Must match `COMMANDS` in build.rs and the permissions in capabilities/chrome.json.
#[macro_export]
macro_rules! host_command_handler {
    () => {
        tauri::generate_handler![
            $crate::commands::nav_state,
            $crate::commands::navigate,
            $crate::commands::go_back,
            $crate::commands::go_forward,
            $crate::commands::reload,
            $crate::commands::pick_share_folder,
            $crate::commands::resolve_share_folder,
        ]
    };
}

#[derive(Default)]
pub struct HostState {
    pub nav: Mutex<Navigator>,
    pub share: Mutex<ShareState>,
    pub guest_label: Mutex<Option<String>>,
    pub overlay: AtomicBool,
}

/// The chrome may only show its own bundled pages (or the Vite dev server in debug builds).
fn is_chrome_url<R: Runtime>(app: &AppHandle<R>, url: &Url) -> bool {
    let bundled = url.scheme() == "tauri" && url.host_str() == Some("localhost");
    let dev_server = cfg!(debug_assertions)
        && app.config().build.dev_url.as_ref().is_some_and(|dev| dev.origin() == url.origin());
    bundled || dev_server
}

/// Title-bar color matching the chrome's light/dark palette (styles.css `--bg`).
fn title_bar_color(theme: Theme) -> Color {
    match theme {
        Theme::Light => Color(244, 244, 245, 255),
        _ => Color(24, 24, 27, 255),
    }
}

fn create_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // Tauri's default macOS title bar style ("Visible") uses a full-size content view, which would
    // place child WebViews underneath the title bar. "Transparent" keeps the client area below it.
    let window = tauri::window::WindowBuilder::new(app, WINDOW_LABEL)
        .title("HyperNavigator - Developer preview")
        .inner_size(1100.0, 780.0)
        .min_inner_size(520.0, 380.0)
        .title_bar_style(TitleBarStyle::Transparent)
        .build()?;
    window.set_background_color(Some(title_bar_color(window.theme()?)))?;

    let nav_app = app.clone();
    let chrome = WebviewBuilder::new(CHROME_LABEL, WebviewUrl::App("index.html".into()))
        .on_navigation(move |url| is_chrome_url(&nav_app, url))
        .on_new_window(|_url, _features| NewWindowResponse::Deny)
        .on_download(|_webview, _event| false)
        .background_color(Color(24, 24, 27, 255));
    let (width, height) = layout::logical_inner_size(&window)?;
    let (chrome_bounds, _) = layout::compute(width, height, false);
    let rect = chrome_bounds.to_rect();
    window.add_child(chrome, rect.position, rect.size)?;

    guest::replace_guest(app, 0, GuestTarget::Placeholder { state: "ready", message: None })
}

fn relayout<R: Runtime>(window: &tauri::Window<R>) {
    if window.label() != WINDOW_LABEL {
        return;
    }
    let state = window.state::<HostState>();
    let guest_label = state.guest_label.lock().unwrap().clone();
    let overlay = state.overlay.load(std::sync::atomic::Ordering::SeqCst);
    let _ = layout::apply(window, guest_label.as_deref(), overlay);
}

/// The compiled config, assets and ACL (capabilities + build.rs app manifest). Expanded exactly once
/// because the macro embeds a `#[no_mangle]` Info.plist symbol; the ACL tests reuse it.
fn context<R: Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(HostState::default())
        .invoke_handler(host_command_handler!())
        .setup(|app| {
            create_main_window(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => relayout(window),
                WindowEvent::ThemeChanged(theme) => {
                    let _ = window.set_background_color(Some(title_bar_color(*theme)));
                }
                _ => {}
            }
        })
        .run(context())
        .expect("error while running HyperNavigator");
}

#[cfg(test)]
mod acl_tests {
    //! Exercises Tauri's real runtime authority, built from this crate's build.rs app manifest and
    //! capabilities/*.json, with the mock runtime. Proves the guest cannot reach Host commands.

    use super::*;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::{LogicalPosition, LogicalSize, Webview};

    const GUEST_URL: &str = "http://127.0.0.1:8765/counter/index.html";
    const ALL_HOST_COMMANDS: [&str; 7] =
        ["nav_state", "navigate", "go_back", "go_forward", "reload", "pick_share_folder", "resolve_share_folder"];
    const PLUGIN_COMMANDS: [&str; 6] = [
        "plugin:fs|read_text_file",
        "plugin:shell|execute",
        "plugin:event|listen",
        "plugin:event|emit",
        "plugin:webview|create_webview",
        "plugin:window|close",
    ];

    fn app() -> tauri::App<MockRuntime> {
        mock_builder()
            .manage(HostState::default())
            .invoke_handler(host_command_handler!())
            .build(context())
            .expect("mock app")
    }

    fn child(app: &tauri::App<MockRuntime>, window_label: &str, label: &str, url: &str) -> Webview<MockRuntime> {
        let window = match app.get_window(window_label) {
            Some(w) => w,
            None => tauri::window::WindowBuilder::new(app, window_label).build().unwrap(),
        };
        window
            .add_child(
                WebviewBuilder::new(label, WebviewUrl::External(url.parse().unwrap())),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(100.0, 100.0),
            )
            .unwrap()
    }

    /// `get_ipc_response` takes `AsRef<Webview>`, which child `Webview`s do not implement themselves.
    struct ChildWebview<'a>(&'a Webview<MockRuntime>);

    impl AsRef<Webview<MockRuntime>> for ChildWebview<'_> {
        fn as_ref(&self) -> &Webview<MockRuntime> {
            self.0
        }
    }

    fn invoke(webview: &Webview<MockRuntime>, cmd: &str, origin: &str) -> Result<(), String> {
        let body = match cmd {
            "navigate" => serde_json::json!({ "url": "not-a-valid-url" }),
            "resolve_share_folder" => serde_json::json!({ "requestId": 1, "approve": false }),
            _ => serde_json::json!({}),
        };
        let request = InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: origin.parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };
        get_ipc_response(&ChildWebview(webview), request).map(|_| ()).map_err(|e| e.to_string())
    }

    fn assert_acl_denied(result: Result<(), String>, what: &str) {
        let err = result.expect_err(&format!("{what} must be denied"));
        assert!(
            err.contains("not allowed") || err.contains("not found") || err.contains("ACL"),
            "{what}: expected an ACL denial, got {err}"
        );
    }

    #[test]
    fn guest_from_dev_origin_cannot_invoke_any_host_or_plugin_command() {
        let app = app();
        let guest = child(&app, WINDOW_LABEL, "guest-1", GUEST_URL);
        for cmd in ALL_HOST_COMMANDS.iter().chain(PLUGIN_COMMANDS.iter()) {
            assert_acl_denied(invoke(&guest, cmd, GUEST_URL), &format!("guest -> {cmd}"));
        }
    }

    #[test]
    fn guest_label_is_denied_even_from_a_local_app_url() {
        // The guest shows the bundled placeholder page between apps; that origin is "local".
        let app = app();
        let guest = child(&app, WINDOW_LABEL, "guest-0", "tauri://localhost/placeholder.html");
        for cmd in ALL_HOST_COMMANDS.iter().chain(PLUGIN_COMMANDS.iter()) {
            assert_acl_denied(invoke(&guest, cmd, "tauri://localhost"), &format!("local guest -> {cmd}"));
        }
    }

    #[test]
    fn chrome_can_call_its_commands_but_no_plugins() {
        let app = app();
        let chrome = child(&app, WINDOW_LABEL, CHROME_LABEL, "tauri://localhost/index.html");
        assert_eq!(invoke(&chrome, "nav_state", "tauri://localhost"), Ok(()));
        // Reaches the command (policy error from the command itself, not an ACL denial).
        let err = invoke(&chrome, "navigate", "tauri://localhost").unwrap_err();
        assert!(err.contains("scheme") || err.contains("valid URL") || err.contains("whitespace"), "{err}");
        for cmd in PLUGIN_COMMANDS.iter().filter(|c| !c.starts_with("plugin:event|listen")) {
            assert_acl_denied(invoke(&chrome, cmd, "tauri://localhost"), &format!("chrome -> {cmd}"));
        }
    }

    #[test]
    fn chrome_label_loaded_with_remote_content_gets_nothing() {
        // If the chrome WebView were ever navigated to the dev origin, it must not keep privileges.
        let app = app();
        let chrome = child(&app, WINDOW_LABEL, CHROME_LABEL, GUEST_URL);
        for cmd in ALL_HOST_COMMANDS {
            assert_acl_denied(invoke(&chrome, cmd, GUEST_URL), &format!("remote chrome -> {cmd}"));
        }
    }

    #[test]
    fn capability_files_are_scoped_to_webview_labels() {
        let chrome: serde_json::Value = serde_json::from_str(include_str!("../capabilities/chrome.json")).unwrap();
        assert_eq!(chrome["webviews"], serde_json::json!([CHROME_LABEL]));
        assert!(chrome.get("windows").is_none(), "a window-wide capability would also cover the guest");
        assert!(chrome.get("remote").is_none());
        let perms: Vec<&str> = chrome["permissions"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
        for p in &perms {
            assert!(!p.contains("default") && !p.contains("allow-all") && !p.contains('*'), "broad permission {p}");
            assert!(!p.starts_with("fs:") && !p.starts_with("shell:"), "plugin permission {p}");
        }
        for cmd in ALL_HOST_COMMANDS {
            assert!(perms.contains(&format!("allow-{}", cmd.replace('_', "-")).as_str()), "missing allow for {cmd}");
        }

        let guest: serde_json::Value = serde_json::from_str(include_str!("../capabilities/guest.json")).unwrap();
        assert_eq!(guest["permissions"], serde_json::json!([]));
        assert!(guest.get("remote").is_none());
    }
}
