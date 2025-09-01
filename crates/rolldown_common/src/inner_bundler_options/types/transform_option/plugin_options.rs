#[derive(Debug, Default, Clone)]
pub struct PluginsOptions {
  pub styled_components: Option<StyledComponentsOptions>,
}

impl From<PluginsOptions> for oxc::transformer::PluginsOptions {
  fn from(options: PluginsOptions) -> Self {
    oxc::transformer::PluginsOptions {
      styled_components: options
        .styled_components
        .map(oxc::transformer::StyledComponentsOptions::from),
    }
  }
}

#[derive(Debug, Default, Clone)]
pub struct StyledComponentsOptions {
  /// Enhances the attached CSS class name on each component with richer output to help
  /// identify your components in the DOM without React DevTools.
  ///
  /// @default true
  pub display_name: Option<bool>,

  /// Controls whether the `displayName` of a component will be prefixed with the filename
  /// to make the component name as unique as possible.
  ///
  /// @default true
  pub file_name: Option<bool>,

  /// Adds a unique identifier to every styled component to avoid checksum mismatches
  /// due to different class generation on the client and server during server-side rendering.
  ///
  /// @default true
  pub ssr: Option<bool>,

  /// Transpiles styled-components tagged template literals to a smaller representation
  /// than what Babel normally creates, helping to reduce bundle size.
  ///
  /// @default true
  pub transpile_template_literals: Option<bool>,

  /// Minifies CSS content by removing all whitespace and comments from your CSS,
  /// keeping valuable bytes out of your bundles.
  ///
  /// @default true
  pub minify: Option<bool>,

  /// Enables transformation of JSX `css` prop when using styled-components.
  ///
  /// **Note: This feature is not yet implemented in oxc.**
  ///
  /// @default true
  pub css_prop: Option<bool>,

  /// Enables "pure annotation" to aid dead code elimination by bundlers.
  ///
  /// @default false
  pub pure: Option<bool>,

  /// Adds a namespace prefix to component identifiers to ensure class names are unique.
  ///
  /// Example: With `namespace: "my-app"`, generates `componentId: "my-app__sc-3rfj0a-1"`
  pub namespace: Option<String>,

  /// List of file names that are considered meaningless for component naming purposes.
  ///
  /// When the `fileName` option is enabled and a component is in a file with a name
  /// from this list, the directory name will be used instead of the file name for
  /// the component's display name.
  ///
  /// @default `["index"]`
  pub meaningless_file_names: Option<Vec<String>>,

  /// Import paths to be considered as styled-components imports at the top level.
  ///
  /// **Note: This feature is not yet implemented in oxc.**
  pub top_level_import_paths: Option<Vec<String>>,
}

impl From<StyledComponentsOptions> for oxc::transformer::StyledComponentsOptions {
  fn from(options: StyledComponentsOptions) -> Self {
    let ops = oxc::transformer::StyledComponentsOptions::default();
    oxc::transformer::StyledComponentsOptions {
      display_name: options.display_name.unwrap_or(ops.display_name),
      file_name: options.file_name.unwrap_or(ops.file_name),
      ssr: options.ssr.unwrap_or(ops.ssr),
      transpile_template_literals: options
        .transpile_template_literals
        .unwrap_or(ops.transpile_template_literals),
      minify: options.minify.unwrap_or(ops.minify),
      css_prop: options.css_prop.unwrap_or(ops.css_prop),
      pure: options.pure.unwrap_or(ops.pure),
      namespace: options.namespace,
      meaningless_file_names: options.meaningless_file_names.unwrap_or(ops.meaningless_file_names),
      top_level_import_paths: options.top_level_import_paths.unwrap_or(ops.top_level_import_paths),
    }
  }
}
