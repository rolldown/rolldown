use std::{future::Future, pin::Pin, sync::Arc};

use rolldown_plugin::{HookLoadOutput, HookTransformOutput, HookUsage, LogWithoutPlugin, Plugin};
use rolldown_plugin_utils::{
  FileToUrlEnv, PublicFileToBuiltUrlEnv, UsizeOrFunction, check_public_file,
  constants::CSSModuleCache, css::is_css_request, find_special_query, inject_query,
  is_special_query, remove_special_query, uri::decode_uri,
};
use rolldown_sourcemap::SourceMap;
use rustc_hash::{FxHashMap, FxHashSet};

type ResolveUrl = dyn (Fn(
    &str,
    Option<&str>,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send + Sync>>)
  + Send
  + Sync;

pub type UrlResolver = dyn Fn(
    String,
    Option<String>,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<(String, Option<String>)>> + Send>>
  + Send
  + Sync;

type CompileCSS = dyn (Fn(
    &str,
    &str,
    Arc<UrlResolver>,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<CompileCSSResult>> + Send + Sync>>)
  + Send
  + Sync;

pub struct CompileCSSResult {
  pub code: String,
  pub map: Option<SourceMap>,
  pub deps: Option<FxHashSet<String>>,
  pub modules: Option<FxHashMap<String, String>>,
}

#[derive(derive_more::Debug)]
pub struct ViteCSSPlugin {
  pub is_lib: bool,
  pub public_dir: String,
  #[debug(skip)]
  pub compile_css: Arc<CompileCSS>,
  #[debug(skip)]
  pub resolve_url: Arc<ResolveUrl>,
  #[debug(skip)]
  pub asset_inline_limit: UsizeOrFunction,
}

impl Plugin for ViteCSSPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("builtin:vite-css")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart | HookUsage::Load | HookUsage::Transform
  }

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    ctx.meta().insert(Arc::new(CSSModuleCache::default()));
    Ok(())
  }

  async fn load(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if is_css_request(args.id) && find_special_query(args.id, b"url").is_some() {
      if rolldown_plugin_utils::css::is_css_module(args.id) {
        return Err(anyhow::anyhow!(
          "?url is not supported with CSS modules. (tried to import '{}')",
          args.id
        ));
      }

      let url = remove_special_query(args.id, b"url");
      let code = rolldown_utils::concat_string!(
        "import ",
        serde_json::to_string(&inject_query(&url, "transform-only"))?,
        "; export default '__VITE_CSS_URL__",
        base64_simd::STANDARD.encode_to_string(url.as_bytes()),
        "__'"
      );
      return Ok(Some(HookLoadOutput { code: code.into(), ..Default::default() }));
    }
    Ok(None)
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !is_css_request(args.id) || is_special_query(args.id) {
      return Ok(None);
    }

    // Compile CSS with the url resolver
    let url_resolver = self.create_url_resolver(Arc::clone(&ctx));
    let CompileCSSResult { code, map, deps, modules } =
      (self.compile_css)(args.id, args.code, url_resolver).await?;

    if let Some(modules) = modules {
      let cache = ctx
        .meta()
        .get::<CSSModuleCache>()
        .ok_or_else(|| anyhow::anyhow!("CSSModuleCache missing"))?;
      cache.inner.insert(args.id.to_owned(), modules);
    }

    if let Some(deps) = deps {
      for dep in deps {
        ctx.add_watch_file(&dep);
      }
    }

    Ok(Some(HookTransformOutput { code: Some(code), map, ..Default::default() }))
  }
}

impl ViteCSSPlugin {
  fn create_url_resolver(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
  ) -> Arc<UrlResolver> {
    let is_lib = self.is_lib;
    let public_dir = self.public_dir.clone();
    let resolve_url = Arc::clone(&self.resolve_url);
    let asset_inline_limit = self.asset_inline_limit.clone();
    Arc::new(move |url: String, importer: Option<String>| {
      let ctx = Arc::clone(&ctx);
      let public_dir = public_dir.clone();
      let asset_inline_limit = asset_inline_limit.clone();
      let resolve_url = Arc::clone(&resolve_url);

      Box::pin(async move {
        let decoded_url = decode_uri(&url);

        // Check if it's a public file
        if check_public_file(&decoded_url, &public_dir).is_some() {
          let env = PublicFileToBuiltUrlEnv::new(&ctx);
          return Ok((env.public_file_to_built_url(&decoded_url), None));
        }

        // Handle fragment in URL
        let (id, fragment) = match decoded_url.split_once('#') {
          Some((id, fragment)) => (id, Some(fragment)),
          None => (decoded_url.as_ref(), None),
        };

        // Try to resolve the URL
        let resolved = resolve_url(id, importer.as_deref()).await?;
        if let Some(mut resolved) = resolved {
          // Append fragment if present
          if let Some(fragment) = fragment {
            resolved = rolldown_utils::concat_string!(resolved, "#", fragment);
          }

          let env = FileToUrlEnv {
            ctx: &ctx,
            root: ctx.cwd(),
            is_lib,
            public_dir: &public_dir,
            asset_inline_limit: &asset_inline_limit,
          };
          return Ok((env.file_to_url(&resolved).await?, Some(resolved)));
        }

        // Check if URL is external
        if !ctx.options().external.call(&decoded_url, Some(id), false).await? {
          let message = rolldown_utils::concat_string!(
            "\n",
            decoded_url,
            " referenced in ",
            id,
            " didn't resolve at build time, it will remain unchanged to be resolved at runtime"
          );
          ctx.warn(LogWithoutPlugin { message, ..Default::default() });
        }

        Ok((url, None))
      })
    })
  }
}
