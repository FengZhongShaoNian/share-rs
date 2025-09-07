use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};
use std::thread::spawn;

#[derive(Serialize, Deserialize)]
pub struct Settings {
    /// Server port
    pub port: u16,

    /// Folder path for saving uploaded file
    pub storage_folder: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 12345,
            storage_folder: dirs::download_dir()
                .unwrap()
                .join("Uploads")
                .to_string_lossy()
                .into_owned(),
        }
    }
}

static SETTINGS: LazyLock<RwLock<Settings>> = LazyLock::new(|| RwLock::new(Settings::new()));

impl Settings {
    fn new() -> Self {
        if !is_setting_file_exists() {
            let settings = Settings::default();
            let _ = store_settings(&settings);
            settings
        } else {
            load_settings().unwrap()
        }
    }

    pub fn init() {
        spawn(|| {
            LazyLock::force(&SETTINGS);
        });
    }

    pub fn global() -> &'static RwLock<Settings> {
        LazyLock::force(&SETTINGS)
    }

    pub fn clone() -> Self {
        let settings = Self::global().read().unwrap();
        Self {
            port: settings.port,
            storage_folder: settings.storage_folder.clone(),
        }
    }

    pub fn update(update_fn: Box<dyn FnOnce(&mut Settings)>) -> Result<()> {
        let result = match Self::global().write() {
            Ok(mut write_guard) => {
                update_fn(write_guard.deref_mut());
                Ok(())
            }
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            return Err(anyhow!("Failed to get write lock, {e}"));
        }

        match Self::global().read() {
            Ok(read_guard) => {
                let settings = &*read_guard;
                let _ = store_settings(settings)?;
                Ok(())
            }
            Err(e) => Err(anyhow!("Failed to get read lock, {e}")),
        }
    }
}

pub fn configuration_dir() -> PathBuf {
    dirs::config_dir().unwrap().join("share-rs")
}

fn setting_file() -> PathBuf {
    configuration_dir().join("settings.json")
}

fn is_setting_file_exists() -> bool {
    fs::exists(setting_file()).unwrap()
}

fn load_settings() -> Result<Settings> {
    let buf = fs::read(setting_file())?;
    let content = String::from_utf8(buf)?;
    let settings = serde_json::from_str(&content)?;
    Ok(settings)
}

fn store_settings(settings: &Settings) -> Result<()> {
    let content = serde_json::to_string(settings)?;
    let setting_file_path = setting_file();
    let parent_directory = setting_file_path.parent().unwrap();
    if !is_setting_file_exists() {
        fs::create_dir_all(parent_directory)?;
    }
    fs::write(setting_file_path, content)?;
    Ok(())
}
