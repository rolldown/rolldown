#[derive(Debug, Default, Clone)]
pub struct DecoratorOptions {
  /// Enables experimental support for decorators, which is a version of decorators that predates the TC39 standardization process.
  ///
  /// Decorators are a language feature which hasn’t yet been fully ratified into the JavaScript specification.
  /// This means that the implementation version in TypeScript may differ from the implementation in JavaScript when it it decided by TC39.
  ///
  /// @see https://www.typescriptlang.org/tsconfig/#experimentalDecorators
  /// @default false
  pub legacy: Option<bool>,

  /// Enables emitting decorator metadata.
  ///
  /// This option the same as [emitDecoratorMetadata](https://www.typescriptlang.org/tsconfig/#emitDecoratorMetadata)
  /// in TypeScript, and it only works when `legacy` is true.
  ///
  /// @see https://www.typescriptlang.org/tsconfig/#emitDecoratorMetadata
  /// @default false
  pub emit_decorator_metadata: Option<bool>,

  /// Controls whether `null` and `undefined` are elided from union `design:type`
  /// metadata emitted when `emit_decorator_metadata` is enabled.
  ///
  /// When `true` (the default), strict TypeScript semantics are preserved, e.g.
  /// `string | null` emits `Object`. When `false`, it matches
  /// `babel-plugin-transform-typescript-metadata` and `tsc` with `--strictNullChecks=false`,
  /// emitting the underlying primitive constructor, e.g. `string | null` emits `String`.
  ///
  /// This option the same as [strictNullChecks](https://www.typescriptlang.org/tsconfig/#strictNullChecks)
  /// in TypeScript, and it only has an effect when `emit_decorator_metadata` is true.
  ///
  /// @see https://www.typescriptlang.org/tsconfig/#strictNullChecks
  /// @default true
  pub strict_null_checks: Option<bool>,
}

impl From<DecoratorOptions> for oxc::transformer::DecoratorOptions {
  fn from(options: DecoratorOptions) -> Self {
    Self {
      legacy: options.legacy.unwrap_or_default(),
      emit_decorator_metadata: options.emit_decorator_metadata.unwrap_or_default(),
      // Oxc defaults `strict_null_checks` to `true` (preserving strict tsc semantics),
      // so mirror that default when the option is not explicitly set.
      strict_null_checks: options.strict_null_checks.unwrap_or(true),
    }
  }
}
