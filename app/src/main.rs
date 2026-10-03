mod bridge;
mod commands;
mod session;

use session::AppState;
use tauri::Manager;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::create_room,
            commands::inspect_invite,
            commands::join_room,
            commands::set_media,
            commands::leave_room
        ])
        .build(tauri::generate_context!())?
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                tauri::async_runtime::block_on(async {
                    app.state::<AppState>().close().await;
                });
            }
        });

    Ok(())
}
