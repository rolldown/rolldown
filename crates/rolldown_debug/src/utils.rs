use std::sync::atomic::AtomicU32;

use oxc::span::CompactStr;

static SESSION_ID_SEED: AtomicU32 = AtomicU32::new(0);
static BUILD_ID_SEED: AtomicU32 = AtomicU32::new(0);

pub fn generate_build_id(build_count: u32) -> CompactStr {
  let seed = BUILD_ID_SEED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

  CompactStr::new(&format!("bid_{seed}_count_{build_count}"))
}

pub fn generate_session_id() -> CompactStr {
  let timestamp = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .expect("Time went backwards")
    .as_millis()
    .to_string();
  let seed = SESSION_ID_SEED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
  CompactStr::new(&format!("sid_{seed}_{timestamp}"))
}
