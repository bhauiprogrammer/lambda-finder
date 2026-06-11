use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const DEFAULT_REPO_ROOT: &str = "/home/bhaup/utec1";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub repo_root: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repo_root: DEFAULT_REPO_ROOT.to_string(),
        }
    }
}

fn config_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("config.json"))
}

pub fn load(app: &AppHandle) -> Config {
    if let Some(path) = config_file(app) {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(cfg) = serde_json::from_slice::<Config>(&bytes) {
                return cfg;
            }
        }
    }
    Config::default()
}

pub fn save(app: &AppHandle, cfg: &Config) -> Result<(), String> {
    let path = config_file(app).ok_or_else(|| "could not resolve app config dir".to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}
