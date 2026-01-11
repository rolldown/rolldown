mod ast_utils;
mod ast_visit;
mod utils;

use std::{borrow::Cow, path::PathBuf, pin::Pin, sync::Arc};

use oxc::ast_visit::VisitMut;
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_plugin_utils::{FileToUrlEnv, UsizeOrFunction};
use rolldown_utils::dataurl::is_data_url;
use sugar_path::SugarPath as _;

pub type TryFsResolve = dyn Fn(&str) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send>>
  + Send
  + Sync;

pub type AssetResolver = dyn Fn(&str, &str) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send>>
  + Send
  + Sync;

#[derive(derive_more::Debug)]
pub struct ViteAssetImportMetaUrlPlugin {
  pub root: PathBuf,
  pub is_lib: bool,
  pub public_dir: String,
  pub client_entry: String,
  #[debug(skip)]
  pub try_fs_resolve: Arc<TryFsResolve>,
  #[debug(skip)]
  pub asset_resolver: Arc<AssetResolver>,
  #[debug(skip)]
  pub asset_inline_limit: UsizeOrFunction,
}

impl Plugin for ViteAssetImportMetaUrlPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-asset-import-meta-url")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if args.id == utils::PRELOAD_HELPER_ID
      || args.id == self.client_entry
      || !utils::contains_asset_import_meta_url(args.code)
    {
      return Ok(None);
    }

    let mut urls = Vec::new();
    let mut s: Option<string_wizard::MagicString> = None;
    let allocator = oxc::allocator::Allocator::default();

    {
      let mut parser_ret =
        oxc::parser::Parser::new(&allocator, args.code, oxc::span::SourceType::default()).parse();
      if parser_ret.panicked
        && let Some(err) =
          parser_ret.errors.iter().find(|e| e.severity == oxc::diagnostics::Severity::Error)
      {
        return Err(anyhow::anyhow!(format!(
          "Failed to parse code in '{}': {:?}",
          args.id, err.message
        )));
      }

      let mut visitor = ast_visit::NewUrlVisitor {
        urls: &mut urls,
        s: &mut s,
        code: args.code,
        ctx: &ctx,
        comments: parser_ret.program.comments,
        current_comment: 0,
      };
      visitor.visit_statements(&mut parser_ret.program.body);
    }

    let env = FileToUrlEnv {
      ctx: &ctx,
      root: &self.root,
      is_lib: self.is_lib,
      public_dir: &self.public_dir,
      asset_inline_limit: &self.asset_inline_limit,
    };

    for (url, span, matched) in urls {
      if is_data_url(&url) {
        continue;
      }
      let file = if url.starts_with('.') {
        let path = PathBuf::from(args.id).parent().unwrap().join(&url).normalize();
        let file = path.to_slash_lossy().into_owned();
        (self.try_fs_resolve)(&file).await?.unwrap_or(file)
      } else {
        (self.asset_resolver)(&url, args.id).await?.unwrap_or_else(|| {
          if let Some(stripped) = url.strip_prefix('/') {
            PathBuf::from(&self.public_dir).join(stripped).to_slash_lossy().into_owned()
          } else {
            let path = PathBuf::from(args.id).parent().unwrap().join(&url).normalize();
            path.to_slash_lossy().into_owned()
          }
        })
      };

      let built_url = if !self.public_dir.is_empty()
        && let Ok(stripped) = PathBuf::from(&file).strip_prefix(&self.public_dir)
      {
        let public_path = format!("/{}", stripped.to_slash_lossy());
        env.file_to_url(&public_path).await
      } else {
        env.file_to_url(&file).await
      };

      let built_url = match built_url {
        Ok(url) => url,
        Err(_) => {
          let message = format!(
            "\n{matched} doesn't exist at build time, it will remain unchanged to be resolved at runtime. If this is intended, you can use the /* @vite-ignore */ comment to suppress this warning."
          );
          ctx.warn(rolldown_plugin::LogWithoutPlugin { message, ..Default::default() });
          url
        }
      };

      s.get_or_insert_with(|| string_wizard::MagicString::new(args.code))
        .update(span.start, span.end, built_url)
        .expect("update should not fail in asset import meta url plugin");
    }

    // TODO: generate source map
    Ok(s.map(|s| rolldown_plugin::HookTransformOutput {
      code: Some(s.to_string()),
      ..Default::default()
    }))
  }
}
