//! Debug-only, opt-in runtime check that the guest cannot reach Host commands or plugins.
//!
//! Run `HN_SECURITY_PROBE=1 npm run tauri:dev` and load an app. After the guest page finishes
//! loading, the Host evaluates [`PROBE_SCRIPT`] inside the guest, i.e. with exactly the privileges
//! of remote app code. The script tries privileged IPC calls and reports the outcome through
//! `document.title` (the guest has no IPC channel back), which is printed as `[hn-security-probe]`.
//! Compiled out of release builds.

use tauri::webview::WebviewBuilder;
use tauri::{AppHandle, Runtime, Webview};

pub const ENV_VAR: &str = "HN_SECURITY_PROBE";
const TITLE_PREFIX: &str = "HN_PROBE:";

pub const PROBE_SCRIPT: &str = r#"(async function () {
  const results = { origin: window.location.origin };
  results.hasTauriGlobal = typeof window.__TAURI__ !== 'undefined';
  const internals = window.__TAURI_INTERNALS__;
  results.hasIpcBridge = typeof internals !== 'undefined' && typeof internals.invoke === 'function';
  const attempts = [
    ['nav_state', {}],
    ['navigate', { url: 'http://127.0.0.1:8765/counter/hypernavigator.json' }],
    ['pick_share_folder', {}],
    ['resolve_share_folder', { requestId: 1, approve: true }],
    ['plugin:fs|read_text_file', { path: '/etc/hosts' }],
    ['plugin:shell|execute', { program: 'sh', args: ['-c', 'id'] }],
    ['plugin:event|listen', { event: 'hn:guest', target: { kind: 'Any' }, handler: 0 }],
    ['plugin:event|emit', { event: 'hn:guest', payload: {} }],
    ['plugin:webview|create_webview', {}],
    ['plugin:window|close', { label: 'main' }],
  ];
  for (const [cmd, args] of attempts) {
    if (!results.hasIpcBridge) { results[cmd] = 'no-ipc-bridge'; continue; }
    try {
      const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), 3000));
      const value = await Promise.race([internals.invoke(cmd, args), timeout]);
      results[cmd] = 'ALLOWED ' + JSON.stringify(value).slice(0, 80);
    } catch (e) {
      // Keep it short: WebKit truncates long document titles.
      results[cmd] = 'denied: ' + String((e && e.message) || e).replace(/\s+/g, ' ').slice(0, 36);
    }
  }
  try { await fetch('file:///etc/hosts'); results.fileUrlFetch = 'ALLOWED'; }
  catch (e) { results.fileUrlFetch = 'denied: ' + String(e).slice(0, 80); }
  document.title = 'HN_PROBE:' + JSON.stringify(results);
  // Escape attempts: both must be refused by the Host (on_new_window / on_navigation).
  setTimeout(function () {
    const popup = window.open('https://example.com/popup', '_blank');
    document.title = 'HN_PROBE:' + JSON.stringify({ popupOpened: popup !== null });
    window.location.href = 'https://example.com/escape';
  }, 300);
})();"#;

fn enabled() -> bool {
    cfg!(debug_assertions) && std::env::var_os(ENV_VAR).is_some()
}

/// Adds the title listener that prints probe results.
pub fn attach<R: Runtime>(builder: WebviewBuilder<R>, _app: AppHandle<R>) -> WebviewBuilder<R> {
    if !enabled() {
        return builder;
    }
    builder.on_document_title_changed(|webview, title| {
        if let Some(report) = title.strip_prefix(TITLE_PREFIX) {
            println!("[hn-security-probe] webview={} {report}", webview.label());
        }
    })
}

/// Runs the probe in a guest page that finished loading.
pub fn run<R: Runtime>(webview: &Webview<R>) {
    if enabled() {
        let _ = webview.eval(PROBE_SCRIPT);
    }
}
