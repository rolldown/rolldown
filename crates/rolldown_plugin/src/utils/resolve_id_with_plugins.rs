use crate::{
  HookResolveDynamicImportArgs, HookResolveIdArgs, HookResolveIdExtraOptions, PluginDriver,
};
use rolldown_common::{ImportKind, ModuleDefFormat, ResolvedId};
use rolldown_resolver::{ResolveError, Resolver};
use std::path::Path;

fn is_http_url(s: &str) -> bool {
  s.starts_with("http://") || s.starts_with("https://") || s.starts_with("//")
}

fn is_data_url(s: &str) -> bool {
  s.trim_start().starts_with("data:")
}

pub async fn resolve_id_with_plugins(
  resolver: &Resolver,
  plugin_driver: &PluginDriver,
  request: &str,
  importer: Option<&str>,
  options: HookResolveIdExtraOptions,
) -> anyhow::Result<Result<ResolvedId, ResolveError>> {
  let import_kind = options.kind;
  if matches!(import_kind, ImportKind::DynamicImport) {
    if let Some(r) = plugin_driver
      .resolve_dynamic_import(&HookResolveDynamicImportArgs {
        importer: importer.map(std::convert::AsRef::as_ref),
        source: request,
      })
      .await?
    {
      return Ok(Ok(ResolvedId {
        module_def_format: ModuleDefFormat::from_path(&r.id),
        ignored: false,
        id: r.id.into(),
        is_external: matches!(r.external, Some(true)),
        package_json: None,
        side_effects: r.side_effects,
      }));
    }
  }
  // Run plugin resolve_id first, if it is None use internal resolver as fallback
  if let Some(r) = plugin_driver
    .resolve_id(&HookResolveIdArgs {
      importer: importer.map(std::convert::AsRef::as_ref),
      specifier: request,
      options,
    })
    .await?
  {
    return Ok(Ok(ResolvedId {
      module_def_format: ModuleDefFormat::from_path(&r.id),
      ignored: false,
      id: r.id.into(),
      is_external: matches!(r.external, Some(true)),
      package_json: None,
      side_effects: r.side_effects,
    }));
  }

  // Auto external http url or data url
  if is_http_url(request) || is_data_url(request) {
    return Ok(Ok(ResolvedId {
      id: request.to_string().into(),
      module_def_format: ModuleDefFormat::Unknown,
      ignored: false,
      is_external: true,
      package_json: None,
      side_effects: None,
    }));
  }

  resolve_id(resolver, request, importer, import_kind)
}

fn resolve_id(
  resolver: &Resolver,
  request: &str,
  importer: Option<&str>,
  import_kind: ImportKind,
) -> anyhow::Result<Result<ResolvedId, ResolveError>> {
  let resolved = resolver.resolve(importer.map(Path::new), request, import_kind)?;

  if let Err(err) = resolved {
    match err {
      ResolveError::Builtin(specifier) => Ok(Ok(ResolvedId {
        id: specifier.into(),
        ignored: false,
        is_external: true,
        module_def_format: ModuleDefFormat::Unknown,
        package_json: None,
        side_effects: None,
      })),
      ResolveError::Ignored(p) => Ok(Ok(ResolvedId {
        //(hyf0) TODO: This `p` doesn't seem to contains `query` or `fragment` of the input. We need to make sure this is ok
        id: p.to_str().expect("Should be valid utf8").into(),
        ignored: true,
        is_external: false,
        module_def_format: ModuleDefFormat::Unknown,
        package_json: None,
        side_effects: None,
      })),
      _ => Ok(Err(err)),
    }
  } else {
    Ok(resolved.map(|resolved| ResolvedId {
      id: resolved.path,
      ignored: false,
      module_def_format: resolved.module_def_format,
      is_external: false,
      package_json: resolved.package_json,
      side_effects: None,
    }))
  }
}
