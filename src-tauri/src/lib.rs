pub mod ai;
pub mod audio;
pub mod backup;
mod commands;
pub mod db;
pub mod error;
pub mod import;
pub mod jobs;
pub mod pipeline;
pub mod pitch;
pub mod research;
pub mod secrets;
pub mod settings;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(jobs::Jobs::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            // Oczekujące przywrócenie z kopii podmieniamy PRZED otwarciem bazy.
            match backup::apply_pending(&dir) {
                Ok(true) => eprintln!("Przywrócono bazę z kopii zapasowej."),
                Ok(false) => {}
                Err(e) => eprintln!("Nie udało się przywrócić kopii: {e}"),
            }
            let db = db::open(&dir.join(backup::DB_NAME))?;
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
            commands::start_pitch,
            commands::save_pitch_edit,
            commands::restore_pitch,
            commands::add_to_examples,
            commands::start_pill_from_dossier,
            commands::list_artists,
            commands::get_artist,
            commands::create_artist,
            commands::update_artist,
            commands::delete_artist,
            commands::accept_proposal,
            commands::reject_proposal,
            commands::restore_pill,
            commands::list_history,
            commands::get_rules,
            commands::save_rules,
            commands::restore_rule_version,
            commands::reset_default_rules,
            commands::import_html_json,
            commands::export_backup,
            commands::restore_backup,
            commands::get_data_dir,
            commands::cancel_job,
            commands::get_track,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
