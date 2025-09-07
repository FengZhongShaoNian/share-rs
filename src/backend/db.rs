pub mod repository;

use crate::migrator::Migrator;
use gpui::{Global, SharedString};
use log::info;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::{MigratorTrait, SchemaManager};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct SqliteDatabaseSource {
    db_file: SharedString,
    initialized: Mutex<bool>,
}

impl SqliteDatabaseSource {
    pub fn new(db_file_path: &str) -> Arc<Self> {
        let db_source = Self {
            db_file: SharedString::new(db_file_path),
            initialized: Mutex::new(false),
        };
        Arc::new(db_source)
    }

    pub async fn connection(self: Arc<Self>) -> anyhow::Result<DatabaseConnection> {
        let mut needs_init = false;
        {
            let mut initialized = self
                .initialized
                .lock()
                .expect("Failed to get db initialize lock");
            if !*initialized {
                needs_init = true;
                *initialized = true;
            }
        }
        if needs_init {
            let db_file = self.db_file.to_string();
            create_sqlite_db(&db_file).await?;
        }

        let connection = Database::connect(format!("sqlite:{}?mode=rw", self.db_file)).await?;
        Ok(connection)
    }
}

pub struct DatabaseSource {
    pub instance: Arc<SqliteDatabaseSource>,
}

impl DatabaseSource {
    pub fn new(db_file_path: &str) -> Self {
        let sqlite = SqliteDatabaseSource::new(db_file_path);
        Self { instance: sqlite }
    }
}

impl Global for DatabaseSource {}

/// Create a sqlite database file under the specified path
async fn create_sqlite_db(db_file_path: &str) -> anyhow::Result<()> {
    info!(
        "Enter create_sqlite_db method, db_file_path:{}",
        db_file_path
    );

    let path = PathBuf::from(db_file_path);
    if let Some(parent_dir) = path.parent() {
        async_fs::create_dir_all(parent_dir).await?;
    }
    let db = Database::connect(format!("sqlite:{}?mode=rwc", db_file_path)).await?;

    let _schema_manager = SchemaManager::new(&db);
    Migrator::up(&db, None).await?;

    Ok(())
}
