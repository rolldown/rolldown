use rolldown_common::BundlerOptions;

pub fn assert_bundled(options: BundlerOptions) {
  let result = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .expect("Failed building the Runtime")
    .block_on(async move {
      let mut bundler = rolldown::Bundler::new(options);
      bundler.generate().await
    });
  assert!(
    result.expect("[Technical Errors]: Failed to bundle.").errors.is_empty(),
    "[Business Errors] Failed to bundle."
  );
}

pub fn assert_bundled_write(options: BundlerOptions) {
  let result = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .expect("Failed building the Runtime")
    .block_on(async move {
      let mut bundler = rolldown::Bundler::new(options);
      bundler.write().await
    });
  assert!(
    result.expect("[Technical Errors]: Failed to bundle.").errors.is_empty(),
    "[Business Errors] Failed to bundle."
  );
}
