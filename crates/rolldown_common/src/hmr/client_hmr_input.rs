/// Per-client input for an HMR push. The server never sees execution state; every
/// client currently receives the full affected factory set, and the per-client
/// ship map (`shipped[C]`) that narrows it lands in a follow-up.
#[derive(Debug)]
pub struct ClientHmrInput<'a> {
  pub client_id: &'a str,
}
