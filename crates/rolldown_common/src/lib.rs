mod asset;
mod chunk;
mod css;
mod ecmascript;
mod file_emitter;
mod inner_bundler_options;
mod module;
mod module_loader;
mod type_aliases;
mod types;

/// This module is to help `rolldown` crate could export types related bundler options easily.
/// `rolldown` crate could use `pub use rolldown_common::bundler_options::*;` to export all types, so we don't need write
/// the same code in `rolldown` crate again.
pub mod bundler_options {
  pub use crate::inner_bundler_options::{
    types::{
      advanced_chunks_options::{AdvancedChunksOptions, MatchGroup},
      checks_options::ChecksOptions,
      comments::Comments,
      es_module_flag::EsModuleFlag,
      experimental_options::ExperimentalOptions,
      filename_template::{FileNameRenderOptions, FilenameTemplate},
      hash_characters::HashCharacters,
      inject_import::InjectImport,
      input_item::InputItem,
      is_external::IsExternal,
      jsx::Jsx,
      module_type::ModuleType,
      normalized_bundler_options::{NormalizedBundlerOptions, SharedNormalizedBundlerOptions},
      output_exports::OutputExports,
      output_format::OutputFormat,
      output_option::{
        AddonFunction, AddonOutputOption, ChunkFilenamesOutputOption, GlobalsOutputOption,
      },
      platform::Platform,
      resolve_options::ResolveOptions,
      source_map_type::SourceMapType,
      sourcemap_ignore_list::SourceMapIgnoreList,
      sourcemap_path_transform::SourceMapPathTransform,
      target::ESTarget,
      treeshake::{InnerOptions, ModuleSideEffects, ModuleSideEffectsRule, TreeshakeOptions},
      watch_option::{NotifyOption, WatchOption},
    },
    BundlerOptions,
  };
}

// We don't want internal position adjustment of files affect users, so all items are exported in the root.
pub use crate::{
  asset::asset_view::AssetView,
  chunk::{
    chunk_table::ChunkTable,
    types::{
      cross_chunk_import_item::CrossChunkImportItem, preliminary_filename::PreliminaryFilename,
    },
    Chunk,
  },
  css::css_view::{CssAssetNameReplacer, CssRenderer, CssView},
  ecmascript::{
    comment_annotation::{get_leading_comment, ROLLDOWN_IGNORE},
    dynamic_import_usage,
    ecma_asset_meta::EcmaAssetMeta,
    ecma_view::{
      generate_replace_this_expr_map, EcmaModuleAstUsage, EcmaView, EcmaViewMeta,
      ImportMetaRolldownAssetReplacer, ThisExprReplaceKind,
    },
    module_idx::ModuleIdx,
    node_builtin_modules::is_existing_node_builtin_modules,
  },
  file_emitter::{EmittedAsset, EmittedChunk, FileEmitter, SharedFileEmitter},
  module::{
    external_module::ExternalModule,
    normal_module::{ModuleRenderArgs, NormalModule},
    Module,
  },
  module_loader::{
    runtime_module_brief::{RuntimeModuleBrief, RUNTIME_MODULE_ID},
    runtime_task_result::RuntimeModuleTaskResult,
    task_result::{EcmaRelated, NormalModuleTaskResult},
    ModuleLoaderMsg,
  },
  types::asset::Asset,
  types::asset_idx::AssetIdx,
  types::asset_meta::InstantiationKind,
  types::ast_scope_idx::AstScopeIdx,
  types::ast_scopes::AstScopes,
  types::bundler_file_system::BundlerFileSystem,
  types::cache::Cache,
  types::chunk_idx::ChunkIdx,
  types::chunk_kind::ChunkKind,
  types::ecma_ast_idx::EcmaAstIdx,
  types::entry_point::{EntryPoint, EntryPointKind},
  types::exports_kind::ExportsKind,
  types::external_module_idx::ExternalModuleIdx,
  types::hybrid_index_vec::HybridIndexVec,
  types::import_kind::ImportKind,
  types::import_record::{
    ImportRecordIdx, ImportRecordMeta, RawImportRecord, ResolvedImportRecord,
  },
  types::importer_record::ImporterRecord,
  types::instantiated_chunk::InstantiatedChunk,
  types::interop::Interop,
  types::member_expr_ref::MemberExprRef,
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
  types::resolved_request_info::ResolvedId,
  types::rollup_pre_rendered_chunk::RollupPreRenderedChunk,
  types::rollup_rendered_chunk::RollupRenderedChunk,
  types::scan_stage::ScanMode,
  types::side_effects,
  types::source_mutation::SourceMutation,
  types::stmt_info::{DebugStmtInfoForTreeShaking, StmtInfo, StmtInfoIdx, StmtInfoMeta, StmtInfos},
  types::str_or_bytes::StrOrBytes,
  types::symbol_name_ref_token::SymbolNameRefToken,
  types::symbol_or_member_expr_ref::SymbolOrMemberExprRef,
  types::symbol_ref::{common_debug_symbol_ref, SymbolRef},
  types::symbol_ref_db::{
    GetLocalDb, GetLocalDbMut, SymbolRefDb, SymbolRefDbForModule, SymbolRefFlags,
  },
  types::watch::{
    BundleEndEventData, BundleEvent, WatcherChangeData, WatcherChangeKind, WatcherEvent,
  },
  types::wrap_kind::WrapKind,
};
pub use bundler_options::*;
