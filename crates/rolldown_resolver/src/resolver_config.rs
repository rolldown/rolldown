use std::path::Path;

use itertools::Itertools;
use oxc_resolver::{EnforceExtension, ResolveOptions as OxcResolverOptions};
use rolldown_common::{Platform, ResolveOptions, TsConfig};
use rolldown_utils::indexmap::FxIndexMap;

/// Configuration for different resolver variants based on import types.
#[expect(clippy::struct_field_names)]
pub struct ResolverConfig {
  pub default_options: OxcResolverOptions,
  pub import_options: OxcResolverOptions,
  pub require_options: OxcResolverOptions,
  pub new_url_options: OxcResolverOptions,
  pub css_options: OxcResolverOptions,
}

impl ResolverConfig {
  /// Builds resolver configurations from user options.
  pub fn build(
    cwd: &Path,
    platform: Platform,
    tsconfig: &TsConfig,
    resolve_options: ResolveOptions,
  ) -> Self {
    // Build condition names
    let mut default_conditions = vec!["default".to_string()];
    default_conditions.extend(resolve_options.condition_names.unwrap_or_default());

    match platform {
      Platform::Node => {
        default_conditions.push("node".to_string());
      }
      Platform::Browser => {
        default_conditions.push("browser".to_string());
      }
      Platform::Neutral => {}
    }

    default_conditions = default_conditions.into_iter().unique().collect();

    let mut import_conditions = vec!["import".to_string()];
    import_conditions.extend(default_conditions.clone());
    let import_conditions = import_conditions.into_iter().unique().collect();

    let mut require_conditions = vec!["require".to_string()];
    require_conditions.extend(default_conditions.clone());
    let require_conditions = require_conditions.into_iter().unique().collect();

    // Build main fields
    let main_fields = resolve_options.main_fields.unwrap_or_else(|| match platform {
      Platform::Node => vec!["main".to_string(), "module".to_string()],
      Platform::Browser => vec!["browser".to_string(), "module".to_string(), "main".to_string()],
      Platform::Neutral => vec![],
    });

    // Build alias fields
    let alias_fields = resolve_options.alias_fields.unwrap_or_else(|| match platform {
      Platform::Browser => vec![vec!["browser".to_string()]],
      _ => vec![],
    });

    // Build extension alias
    let mut extension_alias = resolve_options.extension_alias.unwrap_or_default();
    let mut rewritten_extensions = FxIndexMap::from_iter([
      (".js".to_string(), vec![".js".to_string(), ".ts".to_string(), ".tsx".to_string()]),
      (".jsx".to_string(), vec![".jsx".to_string(), ".ts".to_string(), ".tsx".to_string()]),
      (".mjs".to_string(), vec![".mjs".to_string(), ".mts".to_string()]),
      (".cjs".to_string(), vec![".cjs".to_string(), ".cts".to_string()]),
    ]);
    extension_alias.iter_mut().for_each(|(extension, aliases)| {
      if let Some(rewrites) = rewritten_extensions.shift_remove(extension) {
        aliases.extend(rewrites);
      }
    });
    extension_alias.extend(rewritten_extensions);

    // Build alias
    let alias = resolve_options.alias.map(|alias_map| {
      alias_map
        .iter()
        .map(|(key, values)| {
          (
            key.clone(),
            values
              .iter()
              .map(|value| match value {
                None => oxc_resolver::AliasValue::Ignore,
                Some(path) => oxc_resolver::AliasValue::Path(path.clone()),
              })
              .collect::<Vec<_>>(),
          )
        })
        .collect::<Vec<_>>()
    });

    // Build base options
    let default_options = OxcResolverOptions {
      cwd: Some(cwd.to_path_buf()),
      tsconfig: match tsconfig {
        TsConfig::Auto(v) => v.then_some(oxc_resolver::TsconfigDiscovery::Auto),
        TsConfig::Manual(config_file) => {
          Some(oxc_resolver::TsconfigDiscovery::Manual(oxc_resolver::TsconfigOptions {
            config_file: config_file.clone(),
            references: oxc_resolver::TsconfigReferences::Auto,
          }))
        }
      },
      alias: alias.unwrap_or_default(),
      imports_fields: vec![vec!["imports".to_string()]],
      alias_fields,
      condition_names: default_conditions,
      enforce_extension: EnforceExtension::Auto,
      exports_fields: resolve_options
        .exports_fields
        .unwrap_or_else(|| vec![vec!["exports".to_string()]]),
      extension_alias,
      extensions: resolve_options.extensions.unwrap_or_else(|| {
        [".tsx", ".ts", ".jsx", ".js", ".json"].into_iter().map(str::to_string).collect()
      }),
      fallback: vec![],
      fully_specified: false,
      main_fields,
      main_files: resolve_options.main_files.unwrap_or_else(|| vec!["index".to_string()]),
      modules: resolve_options.modules.unwrap_or_else(|| vec!["node_modules".into()]),
      resolve_to_context: false,
      prefer_relative: false,
      prefer_absolute: false,
      restrictions: vec![],
      roots: vec![],
      symlinks: resolve_options.symlinks.unwrap_or(true),
      builtin_modules: matches!(platform, Platform::Node),
      module_type: true,
      allow_package_exports_in_directory_resolve: false,
      yarn_pnp: resolve_options.yarn_pnp.unwrap_or(false),
    };

    let import_options =
      OxcResolverOptions { condition_names: import_conditions, ..default_options.clone() };

    let require_options =
      OxcResolverOptions { condition_names: require_conditions, ..default_options.clone() };

    let css_options = default_options.clone().with_prefer_relative(true);
    let new_url_options = default_options.clone().with_prefer_relative(true);

    Self { default_options, import_options, require_options, new_url_options, css_options }
  }
}
