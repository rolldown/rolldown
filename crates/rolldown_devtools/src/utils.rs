use std::sync::{Arc, atomic::AtomicU32};

static SESSION_ID_SEED: AtomicU32 = AtomicU32::new(0);
static BUILD_ID_SEED: AtomicU32 = AtomicU32::new(0);

pub fn generate_build_id(build_count: u32) -> Arc<str> {
  let seed = BUILD_ID_SEED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

  format!("bid_{seed}_count_{build_count}").into()
}

pub fn generate_session_id() -> Arc<str> {
  let timestamp = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .expect("Time went backwards")
    .as_millis()
    .to_string();
  let seed = SESSION_ID_SEED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
  format!("sid_{seed}_{timestamp}").into()
}
