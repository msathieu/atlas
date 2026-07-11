use diesel::prelude::*;
use diesel::{
    PgConnection,
    r2d2::{ConnectionManager, Pool},
};
use shared_api::ContainerState;
use shared_server::models::Container;
use shared_server::notify_server;
use shared_server::schema::containers;
use std::error::Error;

pub async fn process_containers(pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get()?;
    let removed = diesel::delete(
        containers::table.filter(
            containers::requested_state
                .eq(ContainerState::NonExistent as i32)
                .and(containers::state.eq(ContainerState::NonExistent as i32)),
        ),
    )
    .execute(&mut db)?;
    if removed > 0 {
        notify_server("WEB:REFRESH").await?;
    }
    let containers: Vec<Container> = containers::table.filter(containers::requested_state.ne(containers::state)).get_results(&mut db)?;
    for _ in containers {}
    Ok(())
}
