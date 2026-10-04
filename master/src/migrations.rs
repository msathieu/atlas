use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use shared_server::{
    hsm::SSHKey,
    models::{DevelopmentEnvironment, Setting},
    schema::{development_environments, settings},
};
use std::error::Error;

pub async fn run_migrations(db: &mut AsyncPgConnection) -> Result<(), Box<dyn Error>> {
    db.build_transaction()
        .serializable()
        .run(async |db| {
            let db_migration_version: Setting = settings::table.find("database_migration").for_update().first(db).await?;
            let version: u32 = db_migration_version.value.parse()?;
            if version < 1 {
                let environments: Vec<DevelopmentEnvironment> = development_environments::table.load(db).await?;
                for environment in environments {
                    if serde_json::from_str::<SSHKey>(&environment.ssh_key).is_ok() {
                        continue;
                    }
                    let key = serde_json::to_string(&SSHKey::Software(environment.ssh_key))?;
                    diesel::update(development_environments::table.find(environment.id))
                        .set(development_environments::ssh_key.eq(key))
                        .execute(db)
                        .await?;
                }
            }
            diesel::update(settings::table.find("database_migration")).set(settings::value.eq("1")).execute(db).await?;
            Ok(())
        })
        .await
}
