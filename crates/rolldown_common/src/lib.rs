mod chunk;
mod css;
mod ecmascript;
mod file_emitter;
mod inner_bundler_options;
mod module;
mod type_aliases;
mod types;

/// This module is to help `rolldown` crate could export types related bundler options easily.
/// `rolldown` crate could use `pub use rolldown_common::bundler_options::*;` to export all types, so we don't need write
/// the same code in `rolldown` crate again.
pub mod bundler_options {
  pub use crate::inner_bundler_options::{
    types::{
      es_module_flag::EsModuleFlag,
      filename_template::{FileNameRenderOptions, FilenameTemplate},
      inject_import::InjectImport,
      input_item::InputItem,
      is_external::IsExternal,
      module_type::ModuleType,
      normalized_bundler_options::NormalizedBundlerOptions,
      output_exports::OutputExports,
      output_format::OutputFormat,
      output_option::{AddonFunction, AddonOutputOption, ChunkFilenamesOutputOption},
      platform::Platform,
      resolve_options::ResolveOptions,
      source_map_type::SourceMapType,
      sourcemap_ignore_list::SourceMapIgnoreList,
      sourcemap_path_transform::SourceMapPathTransform,
      treeshake::{InnerOptions, ModuleSideEffects, TreeshakeOptions},
    },
    BundlerOptions,
  };
}

// We don't want internal position adjustment of files affect users, so all items are exported in the root.
pub use crate::{
  chunk::{
    chunk_table::ChunkTable,
    types::{
      cross_chunk_import_item::CrossChunkImportItem, preliminary_filename::PreliminaryFilename,
    },
    Chunk,
  },
  css::{css_module::CssModule, css_module_idx::CssModuleIdx},
  ecmascript::{ecma_asset_meta::EcmaAssetMeta, ecma_module::EcmaModule, module_idx::ModuleIdx},
  file_emitter::{EmittedAsset, FileEmitter, SharedFileEmitter},
  module::external_module::ExternalModule,
  module::Module,
  types::asset::Asset,
  types::asset_idx::AssetIdx,
  types::asset_meta::InstantiationKind,
  types::asset_source::AssetSource,
  types::ast_scopes::AstScopes,
  types::bundler_file_system::BundlerFileSystem,
  types::chunk_idx::ChunkIdx,
  types::chunk_kind::ChunkKind,
  types::ecma_ast_idx::EcmaAstIdx,
  types::entry_point::{EntryPoint, EntryPointKind},
  types::exports_kind::ExportsKind,
  types::external_module_idx::ExternalModuleIdx,
  types::import_record::{
    ImportKind, ImportRecord, ImportRecordIdx, ImportRecordMeta, RawImportRecord,
  },
  types::importer_record::ImporterRecord,
  types::instantiated_chunk::InstantiatedChunk,
  types::member_expr_ref::MemberExprRef,
  types::module_def_format::ModuleDefFormat,
  types::module_id::ModuleId,
  types::module_idx::LegacyModuleIdx,
  types::module_info::ModuleInfo,
  types::module_table::{IndexExternalModules, IndexModules, ModuleTable},
  types::named_export::LocalExport,
  types::named_import::{NamedImport, Specifier},
  types::output::{Output, OutputAsset},
  types::output_chunk::OutputChunk,
  types::package_json::PackageJson,
  types::rendered_module::RenderedModule,
  types::resolved_export::ResolvedExport,
  types::resolved_request_info::ResolvedId,
  types::rollup_pre_rendered_chunk::RollupPreRenderedChunk,
  types::rollup_rendered_chunk::RollupRenderedChunk,
  types::side_effects,
  types::stmt_info::{DebugStmtInfoForTreeShaking, StmtInfo, StmtInfoIdx, StmtInfos},
  types::str_or_bytes::StrOrBytes,
  types::symbol_or_member_expr_ref::SymbolOrMemberExprRef,
  types::symbol_ref::SymbolRef,
  types::wrap_kind::WrapKind,
};
pub use bundler_options::*;
