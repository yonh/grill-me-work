pub mod active_session;
pub mod agent;
pub mod commands;
pub mod events;
pub mod export;
pub mod git;
pub mod graph;
pub mod llm;
pub mod mcp;
pub mod model;
pub mod prototype;
pub mod scheduler;
pub mod store;
pub mod workspace;

use commands::ActorHandle;
use scheduler::SchedulerMsg;
use store::sqlite::SqliteStore;
use store::Store;
use std::sync::Arc;
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
    let store: Arc<dyn Store> = Arc::new(store);

    let (tx, rx) = mpsc::channel::<SchedulerMsg>(100);
    let tx_clone = tx.clone();

    let store_for_actor = store.clone();
    let store_for_mcp = store.clone();
    let tx_for_mcp = tx.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(ActorHandle { tx })
        .manage(store.clone() as Arc<dyn Store>)
        .setup(move |app| {
            // MCP server so external AI agents can drive Grill-Me.
            let port = mcp::start_mcp_server(
                store_for_mcp,
                tx_for_mcp,
                Some(app.handle().clone()),
            );
            log::info!("[mcp] port={port}");

            let tx_actor = tx_clone;
            let store_actor = store_for_actor;
            let rx = rx;
            tauri::async_runtime::spawn(async move {
                scheduler::run_actor(store_actor, rx, tx_actor).await;
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
            commands::get_outline,
            commands::generate_outline,
            commands::save_outline,
            commands::confirm_outline,
            commands::dismiss_outline,
            commands::request_batch,
            commands::get_pipeline,
            commands::generate_spec,
            commands::save_spec,
            commands::confirm_spec,
            commands::generate_tickets,
            commands::save_tickets,
            commands::confirm_tickets,
            commands::set_ticket_status,
            commands::run_ticket,
            commands::start_round,
            commands::archive_round,
            commands::list_rounds,
            commands::get_round_archive,
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
            commands::get_mcp_info,
            commands::get_active_session,
            commands::set_active_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}