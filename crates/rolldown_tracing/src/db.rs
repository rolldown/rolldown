use std::sync::{Arc, Mutex};

use diesel::{Connection, RunQueryDsl, SqliteConnection, sql_query};

pub struct TracingDb {
  inner: Arc<Mutex<TracingDbImpl>>,
}

impl TracingDb {
  pub fn new(path: &str) -> anyhow::Result<Self> {
    let inner = Arc::new(Mutex::new(TracingDbImpl::new(path)?));
    Ok(TracingDb { inner })
  }
}

pub struct TracingDbImpl {
  conn: SqliteConnection,
}

impl TracingDbImpl {
  pub fn new(path: &str) -> anyhow::Result<Self> {
    let mut conn = SqliteConnection::establish(path)?;
    let init_sql_statements =
      include_str!("../../../packages/tracing/prisma/migrations/20250412064418_init/migration.sql");

    sql_query(init_sql_statements)
      .execute(&mut conn)
      .map_err(|e| anyhow::anyhow!("Failed to execute init SQL: {}", e))?;

    Ok(TracingDbImpl { conn })
  }
}
