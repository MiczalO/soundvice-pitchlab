pub mod ai;
pub mod audio;
mod commands;
pub mod db;
pub mod error;
pub mod jobs;
pub mod pipeline;
pub mod research;
pub mod secrets;
pub mod settings;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(jobs::Jobs::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = db::open(&dir.join("pitchlab.sqlite3"))?;
            app.manage(db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::set_secret,
            commands::has_secret,
            commands::delete_secret,
            commands::get_settings,
            commands::set_setting,
            commands::list_gemini_models,
            commands::probe_gemini_models,
            commands::test_connection,
            commands::prepare_audio,
            commands::start_analysis,
            commands::start_research,
            commands::list_artists,
            commands::get_artist,
            commands::create_artist,
            commands::update_artist,
            commands::delete_artist,
            commands::accept_proposal,
            commands::reject_proposal,
            commands::restore_pill,
            commands::cancel_job,
            commands::get_track,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
