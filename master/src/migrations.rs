use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use shared_server::{models::Setting, schema::settings};
use std::error::Error;

pub async fn run_migrations(db: &mut AsyncPgConnection) -> Result<(), Box<dyn Error>> {
    db.build_transaction()
        .serializable()
        .run(async |db| {
            let db_migration_version: Setting = settings::table.find("database_migration").for_update().first(db).await?;
            let _version: u32 = db_migration_version.value.parse()?;
            diesel::update(settings::table.find("database_migration")).set(settings::value.eq("1")).execute(db).await?;
            Ok(())
        })
        .await
}
