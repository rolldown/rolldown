mod chunk;
mod css;
mod ecmascript;
mod file_emitter;
mod inner_bundler_options;
mod module;
mod types;

/// This module is to help `rolldown` crate could export types related bundler options easily.
/// `rolldown` crate could use `pub use rolldown_common::bundler_options::*;` to export all types, so we don't need write
/// the same code in `rolldown` crate again.
pub mod bundler_options {
  pub use crate::inner_bundler_options::{
    types::{
      filename_template::{FileNameRenderOptions, FilenameTemplate},
      input_item::InputItem,
      is_external::IsExternal,
      module_type::ModuleType,
      normalized_bundler_options::NormalizedBundlerOptions,
      normalized_input_item::NormalizedInputItem,
      output_format::OutputFormat,
      output_option::{AddonFunction, AddonOutputOption},
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
    types::{
      cross_chunk_import_item::CrossChunkImportItem, pre_renderer_chunk::PreRenderedChunk,
      preliminary_filename::PreliminaryFilename,
    },
    Chunk,
  },
  css::{css_module::CssModule, css_module_idx::CssModuleIdx},
  ecmascript::{ecma_module::EcmaModule, module_idx::ModuleIdx},
  file_emitter::{EmittedAsset, FileEmitter, SharedFileEmitter},
  module::external_module::ExternalModule,
  module::Module,
  types::asset_source::AssetSource,
  types::ast_scopes::AstScopes,
  types::bundler_file_system::BundlerFileSystem,
  types::chunk_idx::ChunkIdx,
  types::chunk_kind::ChunkKind,
  types::entry_point::{EntryPoint, EntryPointKind},
  types::exports_kind::ExportsKind,
  types::external_module_idx::ExternalModuleIdx,
  types::import_record::{ImportKind, ImportRecord, ImportRecordIdx, RawImportRecord},
  types::importer_record::ImporterRecord,
  types::js_regex,
  types::module_def_format::ModuleDefFormat,
  types::module_idx::LegacyModuleIdx,
  types::module_info::ModuleInfo,
  types::module_table::{IndexExternalModules, IndexModules, ModuleTable},
  types::named_export::LocalExport,
  types::named_import::{NamedImport, Specifier},
  types::output::{Output, OutputAsset},
  types::output_chunk::OutputChunk,
  types::package_json::PackageJson,
  types::rendered_chunk::RenderedChunk,
  types::rendered_module::RenderedModule,
  types::resolved_export::ResolvedExport,
  types::resolved_path::ResolvedPath,
  types::resolved_request_info::ResolvedRequestInfo,
  types::resource_id::ResourceId,
  types::side_effects,
  types::stmt_info::{DebugStmtInfoForTreeShaking, StmtInfo, StmtInfoIdx, StmtInfos},
  types::symbol_ref::{MemberExprRef, SymbolOrMemberExprRef, SymbolRef},
  types::wrap_kind::WrapKind,
};
pub use bundler_options::*;
