/// The answer to a client's lazy request. `code` and `sourcemap` follow the rules of
/// `HmrPatch`.
#[derive(Debug)]
pub struct HmrLazyChunkOutput {
  pub code: String,
  pub sourcemap: Option<String>,
}
