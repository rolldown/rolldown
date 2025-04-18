use itertools::Either;

#[derive(Debug, Default, Clone)]
pub struct TypeScriptOptions {
  pub jsx_pragma: Option<String>,
  pub jsx_pragma_frag: Option<String>,
  pub only_remove_type_imports: Option<bool>,
  pub allow_namespaces: Option<bool>,
  pub allow_declare_fields: Option<bool>,
  /// Also generate a `.d.ts` declaration file for TypeScript files.
  ///
  /// The source file must be compliant with all
  /// [`isolatedDeclarations`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-5.html#isolated-declarations)
  /// requirements.
  ///
  /// @default false
  pub declaration: Option<IsolatedDeclarationsOptions>,
  /// Rewrite or remove TypeScript import/export declaration extensions.
  ///
  /// - When set to `rewrite`, it will change `.ts`, `.mts`, `.cts` extensions to `.js`, `.mjs`, `.cjs` respectively.
  /// - When set to `remove`, it will remove `.ts`/`.mts`/`.cts`/`.tsx` extension entirely.
  /// - When set to `true`, it's equivalent to `rewrite`.
  /// - When set to `false` or omitted, no changes will be made to the extensions.
  ///
  /// @default false
  pub rewrite_import_extensions: Option<Either<bool, String>>,
}

#[derive(Debug, Default, Clone)]
pub struct IsolatedDeclarationsOptions {
  /// Do not emit declarations for code that has an @internal annotation in its JSDoc comment.
  /// This is an internal compiler option; use at your own risk, because the compiler does not check that the result is valid.
  ///
  /// Default: `false`
  ///
  /// See <https://www.typescriptlang.org/tsconfig/#stripInternal>
  pub strip_internal: Option<bool>,

  pub sourcemap: Option<bool>,
}

impl From<TypeScriptOptions> for oxc::transformer::TypeScriptOptions {
  fn from(options: TypeScriptOptions) -> Self {
    let ops = oxc::transformer::TypeScriptOptions::default();
    Self {
      jsx_pragma: options.jsx_pragma.map(Into::into).unwrap_or(ops.jsx_pragma),
      jsx_pragma_frag: options.jsx_pragma_frag.map(Into::into).unwrap_or(ops.jsx_pragma_frag),
      only_remove_type_imports: options
        .only_remove_type_imports
        .unwrap_or(ops.only_remove_type_imports),
      allow_namespaces: options.allow_namespaces.unwrap_or(ops.allow_namespaces),
      allow_declare_fields: options.allow_declare_fields.unwrap_or(ops.allow_declare_fields),
      optimize_const_enums: false,
      rewrite_import_extensions: options.rewrite_import_extensions.and_then(|value| match value {
        Either::Left(v) => {
          if v {
            Some(oxc::transformer::RewriteExtensionsMode::Rewrite)
          } else {
            None
          }
        }
        Either::Right(v) => match v.as_str() {
          "rewrite" => Some(oxc::transformer::RewriteExtensionsMode::Rewrite),
          "remove" => Some(oxc::transformer::RewriteExtensionsMode::Remove),
          _ => None,
        },
      }),
    }
  }
}
