use std::path::PathBuf;
use std::sync::Arc;

use log::info;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::{MigratorTrait, SchemaManager};
use tokio::sync::OnceCell;

use crate::migrator::Migrator;

/// Lazily-opened SQLite database.
///
/// The first connection runs the migrations; later connections reuse the
/// completed initialization. A failed initialization leaves the cell empty, so
/// the next attempt can retry instead of permanently marking the database as
/// ready.
pub struct SqliteDatabaseSource {
    db_file: String,
    initialized: OnceCell<()>,
}

impl SqliteDatabaseSource {
    pub fn new(db_file_path: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            db_file: db_file_path.into(),
            initialized: OnceCell::new(),
        })
    }

    pub async fn connection(self: Arc<Self>) -> anyhow::Result<DatabaseConnection> {
        self.initialized
            .get_or_try_init(|| create_sqlite_db(self.db_file.to_string()))
            .await?;

        Database::connect(format!("sqlite:{}?mode=rw", self.db_file))
            .await
            .map_err(Into::into)
    }
}

/// GPUI global that gives views access to the application database.
pub struct DatabaseSource {
    pub instance: Arc<SqliteDatabaseSource>,
}

impl DatabaseSource {
    pub fn new(db_file_path: impl Into<String>) -> Self {
        Self {
            instance: SqliteDatabaseSource::new(db_file_path),
        }
    }
}

/// Create the SQLite file and apply all migrations.
async fn create_sqlite_db(db_file_path: String) -> anyhow::Result<()> {
    info!("Initializing SQLite database at {db_file_path}");

    let path = PathBuf::from(&db_file_path);
    if let Some(parent_dir) = path.parent() {
        async_fs::create_dir_all(parent_dir).await?;
    }

    let db = Database::connect(format!("sqlite:{db_file_path}?mode=rwc")).await?;
    let _schema_manager = SchemaManager::new(&db);
    Migrator::up(&db, None).await?;
    Ok(())
}
