use std::time::{SystemTime, UNIX_EPOCH};

pub fn current_utc_timestamp_ms() -> String {
  SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis().to_string()
}
