use rolldown_error::BuildDiagnostic;
use rolldown_sourcemap::SourceMap;

#[derive(Debug)]
pub struct ModuleRenderOutput {
  pub code: String,
  pub map: Option<SourceMap>,
  /// `SOURCEMAP_BROKEN` warnings from collapsing this module's sourcemap chain.
  /// Empty when the chain was collapsed without omissions, or when the map
  /// was produced without going through that collapse.
  pub warnings: Vec<BuildDiagnostic>,
}
