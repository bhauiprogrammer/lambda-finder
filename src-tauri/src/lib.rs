mod config;
mod lambda_finder;
mod pull_branch;

use config::Config;
use lambda_finder::FindResult;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

#[tauri::command]
fn get_config(app: AppHandle) -> Config {
    config::load(&app)
}

#[tauri::command]
fn set_config(app: AppHandle, cfg: Config) -> Result<Config, String> {
    let current = config::load(&app);
    let merged = Config {
        repo_root: if cfg.repo_root.is_empty() {
            current.repo_root
        } else {
            cfg.repo_root
        },
    };
    config::save(&app, &merged)?;
    Ok(merged)
}

#[tauri::command]
fn list_repos() -> Vec<&'static str> {
    lambda_finder::REPOS.iter().map(|r| r.folder).collect()
}

#[tauri::command]
fn find_lambda(
    #[allow(non_snake_case)] repoRoot: String,
    endpoint: String,
    env: String,
) -> Result<FindResult, String> {
    lambda_finder::find_matches(&repoRoot, &endpoint, &env)
}

#[tauri::command]
async fn start_pull(
    app: AppHandle,
    #[allow(non_snake_case)] repoRoot: String,
    branch: String,
) {
    tokio::spawn(async move {
        pull_branch::pull_branch(app, repoRoot, branch).await;
    });
}

#[derive(Serialize)]
struct UpdateInfo {
    version: String,
    current_version: String,
    notes: Option<String>,
}

#[tauri::command]
async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("updater unavailable: {e}"))?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateInfo {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            notes: update.body.clone(),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("update check failed: {e}")),
    }
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("updater unavailable: {e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("update check failed: {e}"))?
        .ok_or_else(|| "No update is available right now.".to_string())?;

    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| format!("install failed: {e}"))?;

    app.restart();
    #[allow(unreachable_code)]
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Ensure the config dir exists on first launch so save() never fails.
            if let Ok(dir) = app.path().app_config_dir() {
                let _ = std::fs::create_dir_all(&dir);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_config,
            list_repos,
            find_lambda,
            start_pull,
            check_for_update,
            install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
