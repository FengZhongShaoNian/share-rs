mod m20250816_000002_create_chunks_table;
mod m20250816_000002_create_shares_table;
mod m20250816_000002_create_uploads_table;

use sea_orm::Database;
use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250816_000002_create_shares_table::Migration),
            Box::new(m20250816_000002_create_uploads_table::Migration),
            Box::new(m20250816_000002_create_chunks_table::Migration),
        ]
    }
}

#[test]
fn test_create_sqlite_database_file() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let project_dir = env!("CARGO_MANIFEST_DIR");
    rt.block_on(async {
        let db = Database::connect(format!(
            "sqlite:{}/target/debug/data.db?mode=rwc",
            project_dir
        ))
        .await
        .unwrap();

        let schema_manager = SchemaManager::new(&db);
        Migrator::refresh(&db).await.unwrap();

        assert!(schema_manager.has_table("shares").await.unwrap());
        assert!(schema_manager.has_table("uploads").await.unwrap());
        assert!(schema_manager.has_table("chunks").await.unwrap());
    });
}
