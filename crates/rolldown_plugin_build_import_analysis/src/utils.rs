use regex::Regex;
use rolldown_common::Output;
use rolldown_plugin::PluginContext;
use rolldown_plugin_utils::constants::{RemovedPureCSSFilesCache, ViteMetadata};
use rustc_hash::{FxHashMap, FxHashSet};
use string_wizard::MagicString;

pub static DYNAMIC_IMPORT_RE: std::sync::LazyLock<Regex> =
  std::sync::LazyLock::new(|| Regex::new(r#"\bimport\s*\(\s*['\"`]"#).unwrap());

pub struct AddDeps<'a, 'b> {
  pub s: &'a mut MagicString<'b>,
  pub ctx: &'a PluginContext,
  pub deps: &'a mut FxHashSet<String>,
  pub owner_filename: String,
  pub analyzed: FxHashSet<String>,
  pub has_removed_pure_css_chunks: &'a mut bool,
  pub expr_range: std::ops::Range<usize>,
}

impl AddDeps<'_, '_> {
  pub fn add_deps(&mut self, bundle: &FxHashMap<String, Output>, filename: &str) {
    if filename == self.owner_filename {
      return;
    }
    if self.analyzed.contains(filename) {
      return;
    }
    self.analyzed.insert(filename.to_string());
    if let Some(chunk) = bundle.get(&self.owner_filename) {
      self.deps.insert(chunk.filename().to_string());
      if let Output::Chunk(chunk) = chunk {
        for dep in &chunk.imports {
          self.add_deps(bundle, dep);
        }
        if let Some(cache) = self.ctx.meta().get::<ViteMetadata>() {
          if let Some(metadata) = cache.inner.get(chunk.preliminary_filename.as_str()) {
            for file in metadata.imported_css.iter() {
              self.deps.insert(file.to_string());
            }
          }
        }
      }
    } else if let Some(chunk) = self
      .ctx
      .meta()
      .get::<RemovedPureCSSFilesCache>()
      .expect("RemovedPureCSSFilesCache is missing")
      .inner
      .get(filename)
    {
      if let Some(cache) = self.ctx.meta().get::<ViteMetadata>() {
        if let Some(metadata) = cache.inner.get(chunk.preliminary_filename.as_str()) {
          if !metadata.imported_css.is_empty() {
            for file in metadata.imported_css.iter() {
              self.deps.insert(file.to_string());
            }
            *self.has_removed_pure_css_chunks = true;
          }
          self.s.update(self.expr_range.start, self.expr_range.end, "Promise.resolve({})");
        }
      }
    }
  }
}
