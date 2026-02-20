use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let conn = manager.get_connection();

        let sql = match backend {
            sea_orm::DatabaseBackend::Postgres => {
                r"
CREATE TABLE IF NOT EXISTS settings (
    tenant_id UUID NOT NULL,
    user_id UUID NOT NULL,
    theme VARCHAR(255),
    language VARCHAR(255),
    PRIMARY KEY (tenant_id, user_id)
);
                "
            }
            sea_orm::DatabaseBackend::MySql => {
                r"
CREATE TABLE IF NOT EXISTS settings (
    tenant_id BINARY(16) NOT NULL,
    user_id BINARY(16) NOT NULL,
    theme VARCHAR(255),
    language VARCHAR(255),
    PRIMARY KEY (tenant_id, user_id)
);
                "
            }
            sea_orm::DatabaseBackend::Sqlite => {
                r"
CREATE TABLE IF NOT EXISTS settings (
    tenant_id BLOB NOT NULL,
    user_id BLOB NOT NULL,
    theme TEXT,
    language TEXT,
    PRIMARY KEY (tenant_id, user_id)
);
                "
            }
        };

        conn.execute_unprepared(sql).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let sql = "DROP TABLE IF EXISTS settings;";
        conn.execute_unprepared(sql).await?;
        Ok(())
    }
}
