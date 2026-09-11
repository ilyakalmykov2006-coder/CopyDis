mod server;
use server::{HostConfig, RoomServer};
use std::sync::Arc;
use tauri::State;

struct AppState {
    server: Arc<RoomServer>,
}
#[tauri::command]
async fn start_host(
    state: State<'_, AppState>,
    room_name: String,
    username: String,
    password: String,
    port: u16,
) -> Result<String, String> {
    state
        .server
        .start(HostConfig {
            room_name,
            username,
            password,
            port,
        })
        .await
        .map_err(|e| e.to_string())
}
fn main() {
    let server = Arc::new(RoomServer::new());
    tauri::Builder::default()
        .manage(AppState { server })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![start_host])
        .run(tauri::generate_context!())
        .expect("failed to run Hush");
}
