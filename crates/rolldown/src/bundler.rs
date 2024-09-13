use std::sync::Arc;

use super::stages::{
  link_stage::{LinkStage, LinkStageOutput},
  scan_stage::ScanStageOutput,
};
use crate::{
  bundler_builder::BundlerBuilder,
  module_loader::hmr_module_loader::HmrModuleLoader,
  stages::{
    generate_stage::{render_hmr_chunk::render_hmr_chunk, GenerateStage},
    scan_stage::ScanStage,
  },
  type_alias::IndexEcmaAst,
  types::{bundle_output::BundleOutput, symbols::Symbols},
  BundlerOptions, SharedOptions, SharedResolver,
};
use anyhow::Result;
use arcstr::ArcStr;
use rolldown_common::{ModuleIdx, ModuleTable, NormalizedBundlerOptions, SharedFileEmitter};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{
  HookBuildEndArgs, HookRenderErrorArgs, SharedPluginDriver, __inner::SharedPluginable,
};
use rustc_hash::FxHashMap;
use tracing_chrome::FlushGuard;

pub struct Bundler {
  pub(crate) closed: bool,
  pub(crate) options: SharedOptions,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) fs: OsFileSystem,
  pub(crate) resolver: SharedResolver,
  pub(crate) file_emitter: SharedFileEmitter,
  pub(crate) _log_guard: Option<FlushGuard>,
  pub(crate) previous_module_table: ModuleTable,
  pub(crate) previous_module_id_to_modules: FxHashMap<ArcStr, ModuleIdx>,
  pub(crate) pervious_index_ecma_ast: IndexEcmaAst,
  pub(crate) pervious_symbols: Symbols,
}

impl Bundler {
  pub fn new(options: BundlerOptions) -> Self {
    BundlerBuilder::default().with_options(options).build()
  }

  pub fn with_plugins(options: BundlerOptions, plugins: Vec<SharedPluginable>) -> Self {
    BundlerBuilder::default().with_options(options).with_plugins(plugins).build()
  }
}

impl Bundler {
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn write(&mut self) -> Result<BundleOutput> {
    let mut output = self.bundle_up(/* is_write */ true).await?;

    self.write_file_to_disk(&output)?;

    self.plugin_driver.write_bundle(&mut output.assets).await?;

    Ok(output)
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate(&mut self) -> Result<BundleOutput> {
    self.bundle_up(/* is_write */ false).await
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&mut self) -> Result<()> {
    if self.closed {
      return Ok(());
    }

    self.closed = true;
    self.plugin_driver.close_bundle().await?;

    Ok(())
  }

  pub async fn scan(&mut self) -> Result<DiagnosableResult<ScanStageOutput>> {
    self.plugin_driver.build_start().await?;

    let mut error_for_build_end_hook = None;

    let scan_stage_output = match ScanStage::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs,
      Arc::clone(&self.resolver),
    )
    .scan()
    .await
    {
      Ok(v) => v,
      Err(err) => {
        // TODO: So far we even call build end hooks on unhandleable errors . But should we call build end hook even for unhandleable errors?
        error_for_build_end_hook = Some(err.to_string());
        self
          .plugin_driver
          .build_end(error_for_build_end_hook.map(|error| HookBuildEndArgs { error }).as_ref())
          .await?;
        self.plugin_driver.close_bundle().await?;
        return Err(err);
      }
    };

    let scan_stage_output = match scan_stage_output {
      Ok(v) => v,
      Err(errs) => {
        if let Some(err_msg) = errs.first().map(ToString::to_string) {
          error_for_build_end_hook = Some(err_msg.clone());
        }
        self
          .plugin_driver
          .build_end(error_for_build_end_hook.map(|error| HookBuildEndArgs { error }).as_ref())
          .await?;
        self.plugin_driver.close_bundle().await?;
        return Ok(Err(errs));
      }
    };

    self
      .plugin_driver
      .build_end(error_for_build_end_hook.map(|error| HookBuildEndArgs { error }).as_ref())
      .await?;

    Ok(Ok(scan_stage_output))
  }

  #[allow(clippy::unused_async)]
  pub async fn hmr_rebuild(&mut self, changed_files: Vec<String>) -> Result<BundleOutput> {
    let hmr_module_loader = HmrModuleLoader::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs,
      Arc::clone(&self.resolver),
      std::mem::take(&mut self.previous_module_id_to_modules),
      std::mem::take(&mut self.previous_module_table),
      std::mem::take(&mut self.pervious_index_ecma_ast),
      std::mem::take(&mut self.pervious_symbols),
    )?;

    let mut hmr_module_loader_output =
      match hmr_module_loader.fetch_changed_files(changed_files).await? {
        Ok(output) => output,
        Err(errors) => {
          return Ok(BundleOutput { warnings: vec![], errors, assets: vec![] });
        }
      };

    let output = render_hmr_chunk(&self.options, &mut hmr_module_loader_output);

    self.write_file_to_disk(&output)?;

    // store last build modules info
    self.previous_module_table = hmr_module_loader_output.module_table;
    self.previous_module_id_to_modules = hmr_module_loader_output.module_id_to_modules;
    self.pervious_index_ecma_ast = hmr_module_loader_output.index_ecma_ast;
    self.pervious_symbols = hmr_module_loader_output.symbols;

    Ok(output)
  }

  fn write_file_to_disk(&self, output: &BundleOutput) -> Result<()> {
    let dir = self.options.cwd.join(&self.options.dir);

    self.fs.create_dir_all(&dir).map_err(|err| {
      anyhow::anyhow!("Could not create directory for output chunks: {:?}", dir).context(err)
    })?;

    for chunk in &output.assets {
      let dest = dir.join(chunk.filename());
      if let Some(p) = dest.parent() {
        if !self.fs.exists(p) {
          self.fs.create_dir_all(p).unwrap();
        }
      };
      self
        .fs
        .write(&dest, chunk.content_as_bytes())
        .map_err(|err| anyhow::anyhow!("Failed to write file in {:?}", dest).context(err))?;
    }

    Ok(())
  }

  async fn try_build(&mut self) -> Result<DiagnosableResult<LinkStageOutput>> {
    let build_info = match self.scan().await? {
      Ok(scan_stage_output) => scan_stage_output,
      Err(errors) => return Ok(Err(errors)),
    };
    Ok(Ok(LinkStage::new(build_info, &self.options).link()))
  }

  #[allow(clippy::missing_transmute_annotations)]
  async fn bundle_up(&mut self, is_write: bool) -> Result<BundleOutput> {
    if self.closed {
      return Err(anyhow::anyhow!(
        "Bundle is already closed, no more calls to 'generate' or 'write' are allowed."
      ));
    }

    let mut link_stage_output = match self.try_build().await? {
      Ok(v) => v,
      Err(errors) => return Ok(BundleOutput { assets: vec![], warnings: vec![], errors }),
    };

    self.plugin_driver.set_module_table(unsafe {
      // Can't ensure the safety here. It's only a temporary solution.
      // - We won't mutate the `module_table` in the generate stage.
      // - We transmute the stacked reference to a static lifetime and it haven't met errors due to we happen
      // to only need to access the `module_table` during this function call.
      std::mem::transmute(&link_stage_output.module_table)
    });

    self.plugin_driver.render_start().await?;

    let mut output = {
      let bundle_output =
        GenerateStage::new(&mut link_stage_output, &self.options, &self.plugin_driver)
          .generate()
          .await;

      if let Some(error) = Self::normalize_error(&bundle_output, |ret| &ret.errors) {
        self.plugin_driver.render_error(&HookRenderErrorArgs { error }).await?;
      }

      bundle_output?
    };

    // Add additional files from build plugins.
    self.file_emitter.add_additional_files(&mut output.assets);

    self.plugin_driver.generate_bundle(&mut output.assets, is_write).await?;

    // store last build modules info
    self.previous_module_table = link_stage_output.module_table;
    self.previous_module_id_to_modules = link_stage_output.module_id_to_modules;
    self.pervious_index_ecma_ast = link_stage_output.ast_table;
    self.pervious_symbols = link_stage_output.symbols;

    Ok(output)
  }

  fn normalize_error<T>(
    ret: &Result<T>,
    errors_fn: impl Fn(&T) -> &[BuildDiagnostic],
  ) -> Option<String> {
    ret.as_ref().map_or_else(
      |error| Some(error.to_string()),
      |ret| errors_fn(ret).first().map(ToString::to_string),
    )
  }

  pub fn options(&self) -> &NormalizedBundlerOptions {
    &self.options
  }
}

fn _test_bundler() {
  #[allow(clippy::needless_pass_by_value)]
  fn _assert_send(_foo: impl Send) {}
  let mut bundler = Bundler::new(BundlerOptions::default());
  let write_fut = bundler.write();
  _assert_send(write_fut);
  let mut bundler = Bundler::new(BundlerOptions::default());
  let generate_fut = bundler.generate();
  _assert_send(generate_fut);
}
