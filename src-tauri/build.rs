fn main() {
    let app_manifest = tauri_build::AppManifest::new().commands(&[
        "list_servers",
        "add_server",
        "remove_server",
        "rename_server",
        "open_server",
        "test_server",
        "test_address",
        "take_open_failure",
        "failed_store_deletions",
        "retry_store_deletion",
        "tabs_snapshot",
        "select_tab",
        "close_tab",
        "board_call",
        "board_state",
        "sign_in_start",
        "sign_in_poll",
        "sign_in_cancel",
        "sign_out",
        "open_in_browser",
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app_manifest))
        .expect("tauri-build failed");
}
