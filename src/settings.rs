use std::fs;
use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};

use anyhow::{Context, Result};
use log::{error, warn};
use serde::{Deserialize, Serialize};

/// User-configurable application settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Port the embedded HTTP server listens on.
    pub port: u16,
    /// Folder where uploaded files are stored.
    pub storage_folder: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 12345,
            storage_folder: dirs::download_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Uploads")
                .to_string_lossy()
                .into_owned(),
        }
    }
}

static SETTINGS: LazyLock<RwLock<Settings>> = LazyLock::new(|| RwLock::new(load_or_default()));

impl Settings {
    /// Return a snapshot of the current settings.
    pub fn get() -> Self {
        match SETTINGS.read() {
            Ok(settings) => settings.clone(),
            Err(poisoned) => {
                warn!("Settings lock was poisoned, recovering the last value");
                poisoned.into_inner().clone()
            }
        }
    }

    /// Apply a change and persist the new settings.
    ///
    /// The write lock is only held while mutating in-memory state; persistence
    /// happens from the snapshot so a slow disk cannot block readers.
    pub fn update(update: impl FnOnce(&mut Settings)) -> Result<()> {
        let snapshot = {
            let mut settings = match SETTINGS.write() {
                Ok(settings) => settings,
                Err(poisoned) => {
                    warn!("Settings lock was poisoned, recovering the last value");
                    poisoned.into_inner()
                }
            };
            update(&mut settings);
            settings.clone()
        };

        store_settings(&snapshot)
    }
}

pub fn configuration_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("share-rs")
}

fn settings_file() -> PathBuf {
    configuration_dir().join("settings.json")
}

fn load_or_default() -> Settings {
    match load_settings() {
        Ok(settings) => settings,
        Err(error) => {
            if settings_file().exists() {
                error!(
                    "Failed to load settings from {}: {error}",
                    settings_file().display()
                );
            }
            Settings::default()
        }
    }
}

fn load_settings() -> Result<Settings> {
    let content = fs::read_to_string(settings_file()).context("Failed to read settings file")?;
    serde_json::from_str(&content).context("Failed to parse settings file")
}

fn store_settings(settings: &Settings) -> Result<()> {
    let path = settings_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create settings directory {}", parent.display()))?;
    }

    let content = serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;
    fs::write(&path, content)
        .with_context(|| format!("Failed to write settings file {}", path.display()))
}
