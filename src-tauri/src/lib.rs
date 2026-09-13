pub mod agent;
pub mod commands;
pub mod events;
pub mod export;
pub mod git;
pub mod graph;
pub mod llm;
pub mod model;
pub mod prototype;
pub mod scheduler;
pub mod store;
pub mod workspace;

use commands::ActorHandle;
use scheduler::SchedulerMsg;
use store::sqlite::SqliteStore;
use store::Store;
use tokio::sync::mpsc;

fn get_db_path() -> String {
    workspace::db_path().to_string_lossy().to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    // Pull sessions + prototypes from Application Support into ~/.grill-work (idempotent).
    workspace::migrate_legacy_on_startup();

    let db_path = get_db_path();
    log::info!("Database path: {}", db_path);

    let store = SqliteStore::new(&db_path).expect("Failed to open database");
    let store_box: Box<dyn Store> = Box::new(store);

    let (tx, rx) = mpsc::channel::<SchedulerMsg>(100);
    let tx_clone = tx.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(ActorHandle { tx })
        .setup(|_app| {
            tauri::async_runtime::spawn(async move {
                scheduler::run_actor(store_box, rx, tx_clone).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_session,
            commands::delete_session,
            commands::get_sessions,
            commands::get_session,
            commands::get_questions,
            commands::answer_question,
            commands::skip_question,
            commands::regenerate_stale,
            commands::dismiss_stale,
            commands::finish_session,
            commands::generate_prototype,
            commands::get_prototype_versions,
            commands::get_prototype_preview_url,
            commands::get_prototype_files,
            commands::read_prototype_file,
            commands::list_agent_tools,
            commands::send_message,
            commands::get_messages,
            commands::export_session,
            commands::get_settings,
            commands::save_settings,
            commands::get_timeline,
            commands::init_timeline,
            commands::get_prototype_dir_path,
            commands::open_prototype_dir,
            commands::list_timeline_commits,
            commands::checkout_timeline,
            commands::fork_timeline,
            commands::get_iteration_graph,
            commands::save_iteration_graph,
            commands::run_iteration_graph,
            commands::cancel_iteration_graph,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
