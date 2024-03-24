mod module;
mod types;

// We don't want internal position adjustment of files affect users, so all items are exported in the root.
pub use crate::{
  module::external_module::ExternalModule,
  module::normal_module::NormalModule,
  types::ast_scope::AstScope,
  types::batched_errors::{BatchedErrors, BatchedResult, IntoBatchedResult},
  types::bundler_file_system::BundlerFileSystem,
  types::chunk_id::ChunkId,
  types::chunk_kind::ChunkKind,
  types::entry_point::{EntryPoint, EntryPointKind},
  types::exports_kind::ExportsKind,
  types::external_module_id::ExternalModuleId,
  types::file_path::{representative_name, FilePath},
  types::import_record::{ImportKind, ImportRecord, ImportRecordId, RawImportRecord},
  types::module_id::ModuleId,
  types::module_path::ResourceId,
  types::module_type::ModuleType,
  types::named_export::LocalExport,
  types::named_import::{NamedImport, Specifier},
  types::normal_module_id::NormalModuleId,
  types::output::{Output, OutputAsset},
  types::output_chunk::OutputChunk,
  types::rendered_chunk::RenderedChunk,
  types::rendered_module::RenderedModule,
  types::resolved_export::ResolvedExport,
  types::resolved_path::ResolvedPath,
  types::stmt_info::{DebugStmtInfoForTreeShaking, StmtInfo, StmtInfoId, StmtInfos},
  types::symbol_ref::SymbolRef,
  types::wrap_kind::WrapKind,
};
