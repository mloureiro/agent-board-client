fn main() {
    let app_manifest = tauri_build::AppManifest::new().commands(&[
        "list_servers",
        "add_server",
        "remove_server",
        "rename_server",
        "open_server",
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app_manifest))
        .expect("tauri-build failed");
}
