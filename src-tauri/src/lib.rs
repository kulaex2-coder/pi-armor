mod content;
mod extensions;
mod launcher;
mod proxy;
mod sounds;

use tauri::Manager;

use launcher::Launcher;

#[tauri::command]
fn start_web_ui(state: tauri::State<Launcher>) -> Result<launcher::WebUiStatus, String> {
    launcher::start(&state)
}

#[tauri::command]
fn stop_web_ui(state: tauri::State<Launcher>) -> Result<launcher::WebUiStatus, String> {
    launcher::stop(&state)
}

#[tauri::command]
fn list_extensions() -> Vec<extensions::ExtensionInfo> {
    extensions::list()
}

#[tauri::command]
fn launch_info() -> launcher::LaunchInfo {
    launcher::info()
}

#[tauri::command]
fn web_appearance() -> String {
    launcher::web_appearance()
}

#[tauri::command]
fn web_settings() -> launcher::WebUiSettings {
    launcher::web_settings()
}

#[tauri::command]
fn set_browser_notifications(enabled: bool) -> Result<launcher::WebUiSettings, String> {
    launcher::set_browser_notifications(enabled)
}

#[tauri::command]
fn ensure_sound_plugin(app: tauri::AppHandle) -> Result<String, String> {
    sounds::ensure_plugin(&app)
}

#[tauri::command]
fn web_status() -> launcher::WebUiStatus {
    launcher::current()
}

#[tauri::command]
fn sound_bootstrap(app: tauri::AppHandle) -> Result<sounds::SoundStatus, String> {
    sounds::bootstrap(&app)
}

#[tauri::command]
fn sound_status(app: tauri::AppHandle) -> Result<sounds::SoundStatus, String> {
    sounds::status(&app)
}

#[tauri::command]
fn sound_set_enabled(app: tauri::AppHandle, enabled: bool) -> Result<sounds::SoundStatus, String> {
    sounds::set_enabled(&app, enabled)
}

#[tauri::command]
fn sound_set_pack(app: tauri::AppHandle, pack_id: String) -> Result<sounds::SoundStatus, String> {
    sounds::set_pack(&app, pack_id)
}

#[tauri::command]
fn theme_status() -> proxy::ThemeStatus {
    proxy::theme_status()
}

#[tauri::command]
fn theme_set(theme_id: String) -> Result<proxy::ThemeStatus, String> {
    proxy::theme_set(theme_id)
}

#[tauri::command]
fn content_status() -> content::ContentStatus {
    content::status()
}

#[tauri::command]
fn content_pick_folder() -> Option<String> {
    content::pick_folder()
}

#[tauri::command]
fn content_set_dir(app: tauri::AppHandle, path: String) -> Result<content::ContentStatus, String> {
    content::set_dir(path)?;
    let _ = sounds::rebind(&app);
    Ok(content::status())
}

#[tauri::command]
fn content_reset_dir(app: tauri::AppHandle) -> Result<content::ContentStatus, String> {
    content::reset_dir()?;
    let _ = sounds::rebind(&app);
    Ok(content::status())
}

#[tauri::command]
fn content_open_dir() -> Result<String, String> {
    content::open_dir()
}

#[tauri::command]
fn sound_preview(
    app: tauri::AppHandle,
    pack_id: Option<String>,
    clip_id: Option<String>,
) -> Result<(), String> {
    sounds::preview(&app, pack_id, clip_id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Launcher::new())
        .invoke_handler(tauri::generate_handler![
            start_web_ui,
            stop_web_ui,
            list_extensions,
            launch_info,
            web_appearance,
            web_settings,
            set_browser_notifications,
            ensure_sound_plugin,
            web_status,
            sound_bootstrap,
            sound_status,
            sound_set_enabled,
            sound_set_pack,
            sound_preview,
            theme_status,
            theme_set,
            content_status,
            content_pick_folder,
            content_set_dir,
            content_reset_dir,
            content_open_dir
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                if let Some(state) = app.try_state::<Launcher>() {
                    let _ = state.stop_owned();
                }
            }
        });
}
