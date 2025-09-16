use crate::bundler_options::SharedNormalizedBundlerOptions;
use bitflags::bitflags;

bitflags! {
  #[derive(Debug, Clone, Copy)]
  /// A flat options struct to avoid passing `&SharedNormalizedBundlerOptions` everywhere.
  /// which also make accessing frequently used options faster.
  pub struct FlatOptions: u16 {
    const IgnoreAnnotations = 1 << 0;
    const JsxPreserve = 1 << 1;
    const IsManualPureFunctionsEmpty = 1 << 2;
    /// If the flag is set, it means the `treeshake.property_read_side_effects` is `Always`.
    /// Otherwise, it is `False`.
    const PropertyReadSideEffects = 1 << 3;
    /// If the flag is set, it means the `treeshake.property_write_side_effects` is `Always`.
    /// Otherwise, it is `False`.
    const PropertyWriteSideEffects = 1 << 4;
    /// If set, ESM import/export syntax should be preserved in the output.
    /// Usage: `!self.options.format.keep_esm_import_export_syntax()`
    const KeepEsmImportExportSyntax = 1 << 5;
    /// If set, the format should call runtime require function.
    /// Usage: `self.options.format.should_call_runtime_require()`
    const ShouldCallRuntimeRequire = 1 << 6;
    /// If set, polyfill require for ESM format when targeting Node platform.
    /// Usage: `self.options.polyfill_require_for_esm_format_with_node_platform()`
    const PolyfillRequireForEsmFormatWithNodePlatform = 1 << 7;
    /// If set, new URL() calls with string literal and import.meta.url should be resolved to assets.
    /// Usage: `self.options.experimental.is_resolve_new_url_to_asset_enabled()`
    const ResolveNewUrlToAssetEnabled = 1 << 8;
    /// If set, inline const optimization is enabled.
    /// Usage: `self.options.optimization.is_inline_const_enabled()`
    const InlineConstEnabled = 1 << 9;
  }
}

impl FlatOptions {
  pub fn from_shared_options(options: &SharedNormalizedBundlerOptions) -> Self {
    let mut flags = Self::empty();
    flags.set(Self::IgnoreAnnotations, !options.treeshake.annotations());
    flags.set(Self::JsxPreserve, options.transform_options.is_jsx_preserve());
    flags
      .set(Self::IsManualPureFunctionsEmpty, options.treeshake.manual_pure_functions().is_none());
    flags.set(
      Self::PropertyReadSideEffects,
      matches!(
        options.treeshake.property_read_side_effects(),
        crate::bundler_options::PropertyReadSideEffects::Always
      ),
    );
    flags.set(
      Self::PropertyWriteSideEffects,
      matches!(
        options.treeshake.property_write_side_effects(),
        crate::bundler_options::PropertyWriteSideEffects::Always
      ),
    );
    flags.set(Self::KeepEsmImportExportSyntax, options.format.keep_esm_import_export_syntax());
    flags.set(Self::ShouldCallRuntimeRequire, options.format.should_call_runtime_require());
    flags.set(
      Self::PolyfillRequireForEsmFormatWithNodePlatform,
      options.polyfill_require_for_esm_format_with_node_platform(),
    );
    flags.set(
      Self::ResolveNewUrlToAssetEnabled,
      options.experimental.is_resolve_new_url_to_asset_enabled(),
    );
    flags.set(Self::InlineConstEnabled, options.optimization.is_inline_const_enabled());
    flags
  }

  #[inline]
  pub fn ignore_annotations(self) -> bool {
    self.contains(Self::IgnoreAnnotations)
  }

  #[inline]
  pub fn jsx_preserve(self) -> bool {
    self.contains(Self::JsxPreserve)
  }

  #[inline]
  pub fn is_manual_pure_functions_empty(self) -> bool {
    self.contains(Self::IsManualPureFunctionsEmpty)
  }

  #[inline]
  pub fn property_read_side_effects(self) -> bool {
    self.contains(Self::PropertyReadSideEffects)
  }

  #[inline]
  pub fn property_write_side_effects(self) -> bool {
    self.contains(Self::PropertyWriteSideEffects)
  }

  #[inline]
  pub fn keep_esm_import_export_syntax(self) -> bool {
    self.contains(Self::KeepEsmImportExportSyntax)
  }

  #[inline]
  pub fn should_call_runtime_require(self) -> bool {
    self.contains(Self::ShouldCallRuntimeRequire)
  }

  #[inline]
  pub fn polyfill_require_for_esm_format_with_node_platform(self) -> bool {
    self.contains(Self::PolyfillRequireForEsmFormatWithNodePlatform)
  }

  #[inline]
  pub fn resolve_new_url_to_asset_enabled(self) -> bool {
    self.contains(Self::ResolveNewUrlToAssetEnabled)
  }

  #[inline]
  pub fn inline_const_enabled(self) -> bool {
    self.contains(Self::InlineConstEnabled)
  }
}
