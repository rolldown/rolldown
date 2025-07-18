mod asset;
mod chunk;
mod css;
mod ecmascript;
mod file_emitter;
mod generated;
mod hmr;
mod inner_bundler_options;
mod module;
mod module_loader;
mod type_aliases;
mod types;

/// This module is to help `rolldown` crate could export types related bundler options easily.
/// `rolldown` crate could use `pub use rolldown_common::bundler_options::*;` to export all types, so we don't need write
/// the same code in `rolldown` crate again.
pub mod bundler_options {
  pub use crate::generated::checks_options::ChecksOptions;
  pub use crate::inner_bundler_options::{
    BundlerOptions,
    types::{
      advanced_chunks_options::{
        AdvancedChunksOptions, ChunkingContext, MatchGroup, MatchGroupName, MatchGroupTest,
      },
      attach_debug_info::AttachDebugInfo,
      chunk_modules_order::ChunkModulesOrderBy,
      debug_options::DebugOptions,
      defer_sync_scan_data_option::DeferSyncScanDataOption,
      es_module_flag::EsModuleFlag,
      experimental_options::ExperimentalOptions,
      filename_template::FilenameTemplate,
      hash_characters::HashCharacters,
      hmr_options::HmrOptions,
      inject_import::InjectImport,
      input_item::InputItem,
      invalidate_js_side_cache::InvalidateJsSideCache,
      is_external::IsExternal,
      legal_comments::LegalComments,
      log_level::LogLevel,
      make_absolute_externals_relative::MakeAbsoluteExternalsRelative,
      mark_module_loaded::MarkModuleLoaded,
      minify_options::{MinifyOptions, MinifyOptionsObject, RawMinifyOptions},
      module_type::ModuleType,
      normalized_bundler_options::{NormalizedBundlerOptions, SharedNormalizedBundlerOptions},
      on_log::{Log, OnLog},
      optimization::OptimizationOption,
      output_exports::OutputExports,
      output_format::OutputFormat,
      output_option::{
        AddonFunction, AddonOutputOption, AssetFilenamesOutputOption, ChunkFilenamesOutputOption,
        GlobalsOutputOption, PreserveEntrySignatures,
      },
      platform::Platform,
      resolve_options::ResolveOptions,
      sanitize_filename::SanitizeFilename,
      source_map_type::SourceMapType,
      sourcemap_ignore_list::SourceMapIgnoreList,
      sourcemap_path_transform::SourceMapPathTransform,
      target::ESTarget,
      transform_options::{JsxPreset, TransformOptions},
      treeshake::{InnerOptions, ModuleSideEffects, ModuleSideEffectsRule, TreeshakeOptions},
      watch_option::{NotifyOption, OnInvalidate, WatchOption},
    },
  };
}

// We don't want internal position adjustment of files affect users, so all items are exported in the root.
pub use crate::{
  asset::asset_view::AssetView,
  chunk::{
    Chunk, ChunkMeta,
    chunk_table::ChunkTable,
    types::{
      AddonRenderContext, chunk_reason_type::ChunkReasonType,
      cross_chunk_import_item::CrossChunkImportItem, preliminary_filename::PreliminaryFilename,
    },
  },
  css::{
    css_asset_meta::CssAssetMeta,
    css_view::{CssAssetNameReplacer, CssRenderer, CssView},
  },
  ecmascript::{
    comment_annotation::{ROLLDOWN_IGNORE, get_leading_comment},
    dynamic_import_usage,
    ecma_asset_meta::EcmaAssetMeta,
    ecma_view::{
      EcmaModuleAstUsage, EcmaView, EcmaViewMeta, ImportMetaRolldownAssetReplacer,
      ThisExprReplaceKind, generate_replace_this_expr_map,
    },
    module_idx::ModuleIdx,
    node_builtin_modules::is_existing_node_builtin_modules,
  },
  file_emitter::{EmittedAsset, EmittedChunk, EmittedChunkInfo, FileEmitter, SharedFileEmitter},
  hmr::{
    hmr_boundary::HmrBoundary,
    hmr_output::{HmrBoundaryOutput, HmrOutput},
  },
  module::{
    Module,
    external_module::ExternalModule,
    normal_module::{ModuleRenderArgs, NormalModule},
  },
  module_loader::{
    AddEntryModuleMsg, ModuleLoaderMsg,
    runtime_module_brief::{RUNTIME_MODULE_ID, RUNTIME_MODULE_KEY, RuntimeModuleBrief},
    runtime_task_result::RuntimeModuleTaskResult,
    task_result::{EcmaRelated, ExternalModuleTaskResult, NormalModuleTaskResult},
  },
  type_aliases::{MemberExprRefResolutionMap, SharedModuleInfoDashMap},
  types::asset::Asset,
  types::asset_meta::{InstantiationKind, SourcemapAssetMeta},
  types::ast_scope_idx::AstScopeIdx,
  types::ast_scopes::AstScopes,
  types::bundler_file_system::BundlerFileSystem,
  types::chunk_idx::ChunkIdx,
  types::chunk_kind::ChunkKind,
  types::constant_value::{ConstExportMeta, ConstantValue},
  types::deconflict::ModuleScopeSymbolIdMap,
  types::defer_sync_scan_data::DeferSyncScanData,
  types::entry_point::{EntryPoint, EntryPointKind},
  types::exports_kind::ExportsKind,
  types::external_module_idx::ExternalModuleIdx,
  types::hmr_info::HmrInfo,
  types::hybrid_index_vec::HybridIndexVec,
  types::import_kind::ImportKind,
  types::import_record::{
    ImportRecordIdx, ImportRecordMeta, RawImportRecord, ResolvedImportRecord,
  },
  types::importer_record::ImporterRecord,
  types::ins_chunk_idx::InsChunkIdx,
  types::instantiated_chunk::InstantiatedChunk,
  types::interop::Interop,
  types::member_expr_ref::MemberExprRef,
  types::member_expr_ref_resolution::MemberExprRefResolution,
  types::module_def_format::ModuleDefFormat,
  types::module_id::ModuleId,
  types::module_idx::LegacyModuleIdx,
  types::module_info::ModuleInfo,
  types::module_render_output::ModuleRenderOutput,
  types::module_table::{IndexExternalModules, IndexModules, ModuleTable},
  types::module_view::ModuleView,
  types::named_export::LocalExport,
  types::named_import::{NamedImport, Specifier},
  types::namespace_alias::NamespaceAlias,
  types::output::{Output, OutputAsset},
  types::output_chunk::{Modules, OutputChunk},
  types::outputs_diagnostics::OutputsDiagnostics,
  types::package_json::PackageJson,
  types::rendered_module::RenderedModule,
  types::resolved_export::ResolvedExport,
  types::resolved_request_info::{ResolvedExternal, ResolvedId},
  types::rollup_pre_rendered_asset::RollupPreRenderedAsset,
  types::rollup_pre_rendered_chunk::RollupPreRenderedChunk,
  types::rollup_rendered_chunk::RollupRenderedChunk,
  types::scan_mode::ScanMode,
  types::side_effects,
  types::source_mutation::SourceMutation,
  types::stmt_info::{DebugStmtInfoForTreeShaking, StmtInfo, StmtInfoIdx, StmtInfoMeta, StmtInfos},
  types::stmt_side_effect::StmtSideEffect,
  types::str_or_bytes::StrOrBytes,
  types::symbol_or_member_expr_ref::{SymbolOrMemberExprRef, TaggedSymbolRef},
  types::symbol_ref::{SymbolRef, common_debug_symbol_ref},
  types::symbol_ref_db::{
    GetLocalDb, GetLocalDbMut, SymbolRefDb, SymbolRefDbForModule, SymbolRefFlags,
  },
  types::watch::WatcherChangeKind,
  types::wrap_kind::WrapKind,
};
pub use bundler_options::*;
