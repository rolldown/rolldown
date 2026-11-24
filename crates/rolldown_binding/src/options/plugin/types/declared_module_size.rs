/// Converts `declaredModuleSize` returned by a JS `load` or `transform` hook.
pub fn declared_module_size_from_js(size: f64) -> anyhow::Result<usize> {
  const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
  if !(0.0..=MAX_SAFE_INTEGER).contains(&size) || size.fract() != 0.0 {
    anyhow::bail!("`declaredModuleSize` must be a non-negative safe integer, received {size}.");
  }
  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "checked above")]
  Ok(size as usize)
}
