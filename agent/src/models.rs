use diesel::prelude::*;

#[derive(Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::settings)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Setting {
    pub name: String,
    pub value: String,
}
