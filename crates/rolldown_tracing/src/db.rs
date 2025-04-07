use diesel::{Connection, SqliteConnection};

// pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn create_sqlite_connect(path: &str) -> anyhow::Result<SqliteConnection> {
  let conn = SqliteConnection::establish(path)?;

  Ok(conn)
}

pub struct TracingDb {
  conn: SqliteConnection,
}
