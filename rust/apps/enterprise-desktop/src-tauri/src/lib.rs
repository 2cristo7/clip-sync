use tauri::Manager;

#[tauri::command]
fn get_connection_status() -> String {
    "disconnected".to_string()
}

#[tauri::command]
fn get_admin_token() -> Option<String> {
    std::env::var("CLIPSYNC_ADMIN_TOKEN").ok()
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_connection_status,
            get_admin_token,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
