#[derive(Default)]
pub struct ClientSession {
  /// Per-client envelope sequence counter.
  pub next_seq: u32,
}
