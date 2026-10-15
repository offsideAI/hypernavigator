// Declaring the app commands here makes Tauri generate `allow-<command>` permissions and, because an
// app ACL manifest now exists, every command invocation is checked against capabilities. Without
// this, commands registered with `invoke_handler` would be callable from every local WebView.
const COMMANDS: &[&str] = &[
    "nav_state",
    "navigate",
    "go_back",
    "go_forward",
    "reload",
    "pick_share_folder",
    "resolve_share_folder",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
