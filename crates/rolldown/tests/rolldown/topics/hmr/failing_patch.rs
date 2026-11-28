use std::{
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use rolldown_common::SourceMapPathTransform;

/// Rendering fails while a source file contains this marker.
const FAIL_MARKER: &str = "FAIL_PATCH";

/// Fails while any source of the sourcemap contains `FAIL_MARKER`: a patch that a client
/// pulls, and a full build that renders the module.
pub fn fail_on_marker() -> SourceMapPathTransform {
  SourceMapPathTransform::new(Arc::new(|sources, sourcemap_path| {
    let dir = Path::new(sourcemap_path).parent().unwrap().to_path_buf();
    Box::pin(async move {
      for source in &sources {
        if std::fs::read_to_string(dir.join(source)).is_ok_and(|code| code.contains(FAIL_MARKER)) {
          anyhow::bail!("simulated failure while rendering an HMR patch");
        }
      }
      Ok(sources)
    })
  }))
}

/// Fails the first patch that a client pulls. Chunks of a full build pass.
pub fn fail_first_patch() -> SourceMapPathTransform {
  let failed = Arc::new(AtomicBool::new(false));
  SourceMapPathTransform::new(Arc::new(move |sources, sourcemap_path| {
    let is_patch =
      sourcemap_path.rsplit(['/', '\\']).next().unwrap().starts_with("__rolldown_hmr_patch");
    let fail = is_patch && !failed.swap(true, Ordering::Relaxed);
    Box::pin(async move {
      if fail {
        anyhow::bail!("simulated failure while rendering an HMR patch");
      }
      Ok(sources)
    })
  }))
}
