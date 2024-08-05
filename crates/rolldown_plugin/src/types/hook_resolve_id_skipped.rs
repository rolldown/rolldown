use arcstr::ArcStr;

use crate::__inner::Pluginable;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct HookResolveIdSkipped {
  pub importer: Option<ArcStr>,
  pub plugin: Arc<dyn Pluginable>,
  pub specifier: ArcStr,
}
