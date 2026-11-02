use std::{
  borrow::Cow,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use arcstr::ArcStr;
use rolldown::{Bundler, BundlerOptions, ExperimentalOptions, InputItem};
use rolldown_common::{ModuleDefFormat, ResolvedExternal, ScanMode};
use rolldown_error::EventKind;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookUsage, Plugin, PluginContext, SharedLoadPluginContext,
};
use rolldown_std_utils::PathExt as _;

#[derive(Debug)]
struct LoadInvalidatedExternal {
  entry_id: ArcStr,
  lib_id: ArcStr,
  is_partial_scan: Arc<AtomicBool>,
}

impl Plugin for LoadInvalidatedExternal {
  fn name(&self) -> Cow<'static, str> {
    "load-invalidated-external".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == "external-lib" {
      return Ok(Some(HookResolveIdOutput {
        id: self.lib_id.clone(),
        external: Some(ResolvedExternal::Absolute),
        ..Default::default()
      }));
    }
    if args.specifier == "./lib.js" || args.specifier == self.lib_id {
      return Ok(Some(HookResolveIdOutput::from_id(self.lib_id.clone())));
    }
    if args.specifier == self.entry_id {
      return Ok(Some(HookResolveIdOutput::from_id(self.entry_id.clone())));
    }
    Ok(None)
  }

  async fn load(&self, ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let code = if args.id == self.entry_id {
      if self.is_partial_scan.load(Ordering::Relaxed) {
        ctx.load(&self.lib_id, None, ModuleDefFormat::Unknown).await?;
        "import { value } from './lib.js'; console.log(value);"
      } else {
        "import { value } from 'external-lib'; console.log(value);"
      }
    } else if args.id == self.lib_id {
      // Keep the normal task pending while the entry calls `this.load` for the same id.
      tokio::time::sleep(Duration::from_millis(50)).await;
      "export const value = 42;"
    } else {
      return Ok(None);
    };
    Ok(Some(HookLoadOutput { code: code.into(), ..Default::default() }))
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn partial_scan_uses_current_module_kind() {
  let cwd = std::env::current_dir().unwrap();
  let entry_id: ArcStr = cwd.join("entry.js").expect_to_str().into();
  let lib_id: ArcStr = cwd.join("lib.js").expect_to_str().into();

  for invalidate_lib in [false, true] {
    let is_partial_scan = Arc::new(AtomicBool::new(false));
    let mut bundler = Bundler::with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: None, import: entry_id.to_string() }]),
        experimental: Some(ExperimentalOptions {
          incremental_build: Some(true),
          ..Default::default()
        }),
        ..Default::default()
      },
      vec![Plugin::new_shared(LoadInvalidatedExternal {
        entry_id: entry_id.clone(),
        lib_id: lib_id.clone(),
        is_partial_scan: Arc::clone(&is_partial_scan),
      })],
    )
    .unwrap();
    bundler.incremental_generate(ScanMode::Full).await.unwrap();
    is_partial_scan.store(true, Ordering::Relaxed);

    let changed_ids =
      if invalidate_lib { vec![lib_id.clone(), entry_id.clone()] } else { vec![entry_id.clone()] };
    let result = tokio::time::timeout(
      Duration::from_secs(5),
      bundler.incremental_generate(ScanMode::Partial(changed_ids)),
    )
    .await
    .expect("`this.load` must not hang a partial scan");

    if invalidate_lib {
      result.expect("The current normal task must override the cached external kind");
    }
  }
}

/// entry.js imports lib.js as external. In the partial scan, other.js also imports lib.js,
/// as a normal module. entry.js does not change, so a fresh build of these sources also fails.
#[derive(Debug)]
struct ChangeSecondImporter {
  entry_id: ArcStr,
  other_id: ArcStr,
  lib_id: ArcStr,
  is_partial_scan: Arc<AtomicBool>,
}

impl Plugin for ChangeSecondImporter {
  fn name(&self) -> Cow<'static, str> {
    "change-second-importer".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == "external-lib" {
      return Ok(Some(HookResolveIdOutput {
        id: self.lib_id.clone(),
        external: Some(ResolvedExternal::Absolute),
        ..Default::default()
      }));
    }
    let id = if args.specifier == "./lib.js" {
      &self.lib_id
    } else if args.specifier == "./other.js" || args.specifier == self.other_id {
      &self.other_id
    } else if args.specifier == self.entry_id {
      &self.entry_id
    } else {
      return Ok(None);
    };
    Ok(Some(HookResolveIdOutput::from_id(id.clone())))
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let code = if args.id == self.entry_id {
      "import 'external-lib'; import './other.js';"
    } else if args.id == self.other_id {
      if self.is_partial_scan.load(Ordering::Relaxed) { "import './lib.js';" } else { "" }
    } else if args.id == self.lib_id {
      "export const value = 42;"
    } else {
      return Ok(None);
    };
    Ok(Some(HookLoadOutput { code: code.into(), ..Default::default() }))
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn kind_from_previous_build_suggests_full_rebuild() {
  let cwd = std::env::current_dir().unwrap();
  let entry_id: ArcStr = cwd.join("entry.js").expect_to_str().into();
  let other_id: ArcStr = cwd.join("other.js").expect_to_str().into();
  let lib_id: ArcStr = cwd.join("lib.js").expect_to_str().into();
  let is_partial_scan = Arc::new(AtomicBool::new(false));
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem { name: None, import: entry_id.to_string() }]),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![Plugin::new_shared(ChangeSecondImporter {
      entry_id,
      other_id: other_id.clone(),
      lib_id,
      is_partial_scan: Arc::clone(&is_partial_scan),
    })],
  )
  .unwrap();
  bundler.incremental_generate(ScanMode::Full).await.unwrap();
  is_partial_scan.store(true, Ordering::Relaxed);

  let Err(errors) = bundler.incremental_generate(ScanMode::Partial(vec![other_id])).await else {
    panic!("entry.js and other.js resolve lib.js to different kinds");
  };
  assert!(!errors.is_empty());
  for error in errors.iter() {
    assert!(matches!(error.kind(), EventKind::ExternalModulesCannotBeTransformedToModulesError));
    assert!(error.to_diagnostic().to_string().contains("a full rebuild fixes this error"));
  }
}
