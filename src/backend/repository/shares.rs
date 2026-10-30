use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel};
use snowflaked::sync::Generator;

use crate::backend::entities::shares;
use crate::mimes::detect_mime_type;

/// A file exposed by the HTTP server and shown in the desktop file list.
///
/// Keeping this type separate from the SeaORM model lets the UI depend on a
/// small, stable shape instead of the database schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareFile {
    pub id: i64,
    pub name: String,
    pub path: PathBuf,
    pub mime_type: String,
}

impl ShareFile {
    fn from_model(model: shares::Model) -> Self {
        Self {
            id: model.id,
            name: model.file_name,
            path: PathBuf::from(model.file_path),
            mime_type: model.mime_type,
        }
    }
}

static ID_GENERATOR: LazyLock<Mutex<Generator>> = LazyLock::new(|| Mutex::new(Generator::new(0)));

fn next_id() -> i64 {
    let generator = match ID_GENERATOR.lock() {
        Ok(generator) => generator,
        Err(poisoned) => poisoned.into_inner(),
    };
    generator.generate()
}

/// Return a shared file by id.
pub async fn get(connection: &DatabaseConnection, id: i64) -> Result<Option<ShareFile>> {
    let model = shares::Entity::find_by_id(id)
        .one(connection)
        .await
        .with_context(|| format!("Failed to query shared file {id}"))?;
    Ok(model.map(ShareFile::from_model))
}

/// Return every shared file, oldest first.
pub async fn list(connection: &DatabaseConnection) -> Result<Vec<ShareFile>> {
    let models = shares::Entity::find()
        .all(connection)
        .await
        .context("Failed to query shared files")?;

    Ok(models.into_iter().map(ShareFile::from_model).collect())
}

/// Add a file to the share list.
///
/// The path is canonicalized first so the web server and the desktop file
/// manager always open the same file.
pub async fn add(connection: &DatabaseConnection, path: &Path) -> Result<ShareFile> {
    let path = async_fs::canonicalize(path)
        .await
        .with_context(|| format!("Failed to canonicalize {}", path.display()))?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("Invalid file name for {}", path.display()))?
        .to_string();

    let path_string = path.to_string_lossy().into_owned();
    let share = shares::Model {
        id: next_id(),
        file_name,
        mime_type: detect_mime_type(&path_string),
        file_path: path_string,
    }
    .into_active_model();

    let model = share
        .insert(connection)
        .await
        .with_context(|| format!("Failed to insert shared file {}", path.display()))?;

    info!("Added shared file: {}", path.display());
    Ok(ShareFile::from_model(model))
}

/// Add several files, skipping the ones that cannot be added.
pub async fn add_many<I>(connection: &DatabaseConnection, paths: I) -> Vec<ShareFile>
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut added = Vec::new();
    for path in paths {
        match add(connection, &path).await {
            Ok(file) => added.push(file),
            Err(error) => warn!("Failed to add {}: {error:#}", path.display()),
        }
    }
    added
}

/// Remove a file from the share list.
pub async fn remove(connection: &DatabaseConnection, id: i64) -> Result<()> {
    shares::Entity::delete_by_id(id)
        .exec(connection)
        .await
        .with_context(|| format!("Failed to delete shared file {id}"))?;
    Ok(())
}
