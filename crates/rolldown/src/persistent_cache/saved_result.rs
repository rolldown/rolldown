use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_common::{
  EmittedAsset, EmittedChunk, EmittedPrebuiltChunk, ImportAttributes, ImportKind, Log, LogLevel,
  LogLocation, ModuleDefFormat, ModuleId, ModuleType, PackageJson, PluginIdx,
  PreserveEntrySignatures, ResolvedExternal, ResolvedId, SourcemapChainElement, StrOrBytes,
  side_effects::HookSideEffects,
};
use rolldown_plugin::{__inner::infer_module_def_format, RecordedAction};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::url::clean_url;

use super::codec::{Decoder, Encoder};

/// Bumped on every change to the encoding below. A saved result in another format is not found.
pub const SAVED_RESULT_FORMAT: u32 = 9;

/// What the persistent cache saves for a module: the state after `transform`, the resolve result
/// of each import, and the actions of the skipped hooks that Rolldown does again on reuse.
#[derive(Debug)]
pub struct SavedResult {
  pub source: StrOrBytes,
  pub module_type: ModuleType,
  /// Only the value that `load` or `transform` returned. The value from the importer's
  /// `resolveId` is part of the importer's resolve results.
  pub hook_side_effects: Option<HookSideEffects>,
  pub sourcemap_chain: Vec<SourcemapChainElement>,
  pub is_read_from_disk: bool,
  pub resolved: Vec<SavedResolve>,
  /// The actions of the `load` and `transform` hooks. Done again before the module is parsed.
  pub actions: Vec<RecordedAction>,
  /// The actions of the resolve hooks for the imports of the module. Done again after the
  /// imports are resolved, because a `transform_ast` hook runs between the two groups.
  pub resolve_actions: Vec<RecordedAction>,
}

/// Imports with the same specifier, kind and attributes share one resolve result, as in
/// `resolve_dependencies`.
#[derive(Debug)]
pub struct SavedResolve {
  pub specifier: String,
  pub kind: ImportKind,
  pub attributes: ImportAttributes,
  /// `None` stands for `ResolveError::NotFound`.
  pub resolved: Option<SavedResolvedId>,
}

/// The fields of a `ResolvedId` that cannot be filled in again. `module_def_format` and the
/// `package.json` itself are filled in again on reuse.
#[derive(Debug)]
pub struct SavedResolvedId {
  pub module_def_format: DefFormatFrom,
  pub id: ModuleId,
  pub attributes: ImportAttributes,
  pub external: ResolvedExternal,
  pub normalize_external_id: Option<bool>,
  pub package_json_path: Option<String>,
  pub side_effects: Option<HookSideEffects>,
  pub is_external_without_side_effects: bool,
}

/// How to fill in `module_def_format` again, so that a change to the `type` field of
/// `package.json` takes effect. A `resolveId` hook gives a format inferred from the whole id. The
/// built-in resolver infers it from the path without `?query` and `#fragment`.
#[derive(Debug, Clone, Copy)]
pub enum DefFormatFrom {
  Id,
  IdWithoutQuery,
  /// Neither inference gives the value, for example the runtime module. It is kept as it was.
  Fixed(ModuleDefFormat),
}

impl DefFormatFrom {
  fn of(resolved_id: &ResolvedId) -> Self {
    let raw_id = resolved_id.id.raw_id();
    let package_json = resolved_id.package_json.as_ref();
    if infer_module_def_format(raw_id, package_json) == resolved_id.module_def_format {
      Self::Id
    } else if infer_module_def_format(clean_url(raw_id), package_json)
      == resolved_id.module_def_format
    {
      Self::IdWithoutQuery
    } else {
      Self::Fixed(resolved_id.module_def_format)
    }
  }

  pub fn infer(self, raw_id: &str, package_json: Option<&Arc<PackageJson>>) -> ModuleDefFormat {
    match self {
      Self::Id => infer_module_def_format(raw_id, package_json),
      Self::IdWithoutQuery => infer_module_def_format(clean_url(raw_id), package_json),
      Self::Fixed(format) => format,
    }
  }
}

impl SavedResolvedId {
  pub fn from_resolved_id(resolved_id: &ResolvedId) -> Self {
    Self {
      module_def_format: DefFormatFrom::of(resolved_id),
      id: resolved_id.id.clone(),
      attributes: resolved_id.attributes.clone(),
      external: resolved_id.external,
      normalize_external_id: resolved_id.normalize_external_id,
      package_json_path: resolved_id
        .package_json
        .as_ref()
        .map(|package_json| package_json.realpath().to_string_lossy().into_owned()),
      side_effects: resolved_id.side_effects,
      is_external_without_side_effects: resolved_id.is_external_without_side_effects,
    }
  }
}

impl SavedResult {
  pub fn encode(&self) -> Vec<u8> {
    let mut e = Encoder::new();
    e.u32(SAVED_RESULT_FORMAT);
    write_str_or_bytes(&mut e, &self.source);
    write_module_type(&mut e, &self.module_type);
    e.opt(self.hook_side_effects, write_hook_side_effects);
    e.seq(self.sourcemap_chain.iter(), write_sourcemap_element);
    e.bool(self.is_read_from_disk);
    e.seq(self.resolved.iter(), |e, saved| {
      e.str(&saved.specifier);
      e.u8(import_kind_tag(saved.kind));
      write_attributes(e, &saved.attributes);
      e.opt(saved.resolved.as_ref(), write_saved_resolved_id);
    });
    e.seq(self.actions.iter(), write_action);
    e.seq(self.resolve_actions.iter(), write_action);
    e.finish()
  }

  pub fn decode(bytes: &[u8]) -> Option<Self> {
    let mut d = Decoder::new(bytes);
    if d.u32()? != SAVED_RESULT_FORMAT {
      return None;
    }
    let result = Self {
      source: read_str_or_bytes(&mut d)?,
      module_type: read_module_type(&mut d)?,
      hook_side_effects: d.opt(read_hook_side_effects)?,
      sourcemap_chain: d.seq(read_sourcemap_element)?,
      is_read_from_disk: d.bool()?,
      resolved: d.seq(|d| {
        Some(SavedResolve {
          specifier: d.string()?,
          kind: import_kind_from_tag(d.u8()?)?,
          attributes: read_attributes(d)?,
          resolved: d.opt(read_saved_resolved_id)?,
        })
      })?,
      actions: d.seq(read_action)?,
      resolve_actions: d.seq(read_action)?,
    };
    d.is_empty().then_some(result)
  }
}

fn write_str_or_bytes(e: &mut Encoder, value: &StrOrBytes) {
  match value {
    StrOrBytes::Str(s) => {
      e.u8(0);
      e.str(s);
    }
    StrOrBytes::Bytes(b) => {
      e.u8(1);
      e.bytes(b);
    }
  }
}

fn read_str_or_bytes(d: &mut Decoder) -> Option<StrOrBytes> {
  match d.u8()? {
    0 => Some(StrOrBytes::Str(d.string()?)),
    1 => Some(StrOrBytes::Bytes(d.bytes()?.to_vec())),
    _ => None,
  }
}

fn write_module_type(e: &mut Encoder, value: &ModuleType) {
  // A custom type can have the name of a built-in one, so it gets its own tag.
  match value {
    ModuleType::Custom(name) => {
      e.u8(1);
      e.str(name);
    }
    known => {
      e.u8(0);
      e.str(&known.to_string());
    }
  }
}

fn read_module_type(d: &mut Decoder) -> Option<ModuleType> {
  match d.u8()? {
    0 => ModuleType::from_known_str(d.str()?).ok(),
    1 => Some(ModuleType::Custom(d.string()?)),
    _ => None,
  }
}

fn write_hook_side_effects(e: &mut Encoder, value: HookSideEffects) {
  e.u8(match value {
    HookSideEffects::True => 0,
    HookSideEffects::False => 1,
    HookSideEffects::NoTreeshake => 2,
  });
}

fn read_hook_side_effects(d: &mut Decoder) -> Option<HookSideEffects> {
  match d.u8()? {
    0 => Some(HookSideEffects::True),
    1 => Some(HookSideEffects::False),
    2 => Some(HookSideEffects::NoTreeshake),
    _ => None,
  }
}

fn write_sourcemap(e: &mut Encoder, map: &SourceMap) {
  e.str(&map.to_json_string());
}

fn read_sourcemap(d: &mut Decoder) -> Option<SourceMap> {
  rolldown_sourcemap::sourcemap_from_json(d.str()?)
}

fn write_plugin_idx(e: &mut Encoder, idx: PluginIdx) {
  e.u32(idx.raw());
}

fn read_plugin_idx(d: &mut Decoder) -> Option<PluginIdx> {
  Some(PluginIdx::from_raw(d.u32()?))
}

fn write_sourcemap_element(e: &mut Encoder, element: &SourcemapChainElement) {
  match element {
    SourcemapChainElement::Transform((plugin_idx, map)) => {
      e.u8(0);
      write_plugin_idx(e, *plugin_idx);
      write_sourcemap(e, map);
    }
    SourcemapChainElement::Load(map) => {
      e.u8(1);
      write_sourcemap(e, map);
    }
    SourcemapChainElement::Omitted { plugin_idx, plugin_name } => {
      e.u8(2);
      write_plugin_idx(e, *plugin_idx);
      e.str(plugin_name);
    }
    SourcemapChainElement::Null { plugin_idx, original_content } => {
      e.u8(3);
      write_plugin_idx(e, *plugin_idx);
      e.str(original_content);
    }
  }
}

fn read_sourcemap_element(d: &mut Decoder) -> Option<SourcemapChainElement> {
  match d.u8()? {
    0 => {
      let plugin_idx = read_plugin_idx(d)?;
      Some(SourcemapChainElement::Transform((plugin_idx, read_sourcemap(d)?)))
    }
    1 => Some(SourcemapChainElement::Load(read_sourcemap(d)?)),
    2 => Some(SourcemapChainElement::Omitted {
      plugin_idx: read_plugin_idx(d)?,
      plugin_name: d.str()?.into(),
    }),
    3 => Some(SourcemapChainElement::Null {
      plugin_idx: read_plugin_idx(d)?,
      original_content: d.str()?.into(),
    }),
    _ => None,
  }
}

fn import_kind_tag(kind: ImportKind) -> u8 {
  match kind {
    ImportKind::Import => 0,
    ImportKind::DynamicImport => 1,
    ImportKind::Require => 2,
    ImportKind::AtImport => 3,
    ImportKind::UrlImport => 4,
    ImportKind::NewUrl => 5,
    ImportKind::HotAccept => 6,
  }
}

fn import_kind_from_tag(tag: u8) -> Option<ImportKind> {
  Some(match tag {
    0 => ImportKind::Import,
    1 => ImportKind::DynamicImport,
    2 => ImportKind::Require,
    3 => ImportKind::AtImport,
    4 => ImportKind::UrlImport,
    5 => ImportKind::NewUrl,
    6 => ImportKind::HotAccept,
    _ => return None,
  })
}

fn write_attributes(e: &mut Encoder, attributes: &ImportAttributes) {
  let entries: Vec<_> = attributes.iter().collect();
  e.seq(entries.into_iter(), |e, (key, value)| {
    e.str(key);
    e.str(value);
  });
}

fn read_attributes(d: &mut Decoder) -> Option<ImportAttributes> {
  let entries = d.seq(|d| Some((d.string()?, d.string()?)))?;
  Some(ImportAttributes::from_entries(entries))
}

fn write_def_format_from(e: &mut Encoder, value: DefFormatFrom) {
  match value {
    DefFormatFrom::Id => e.u8(0),
    DefFormatFrom::IdWithoutQuery => e.u8(1),
    DefFormatFrom::Fixed(format) => {
      e.u8(2);
      e.u8(match format {
        ModuleDefFormat::Unknown => 0,
        ModuleDefFormat::Cjs => 1,
        ModuleDefFormat::Cts => 2,
        ModuleDefFormat::CjsPackageJson => 3,
        ModuleDefFormat::EsmMjs => 4,
        ModuleDefFormat::EsmMts => 5,
        ModuleDefFormat::EsmPackageJson => 6,
      });
    }
  }
}

fn read_def_format_from(d: &mut Decoder) -> Option<DefFormatFrom> {
  Some(match d.u8()? {
    0 => DefFormatFrom::Id,
    1 => DefFormatFrom::IdWithoutQuery,
    2 => DefFormatFrom::Fixed(match d.u8()? {
      0 => ModuleDefFormat::Unknown,
      1 => ModuleDefFormat::Cjs,
      2 => ModuleDefFormat::Cts,
      3 => ModuleDefFormat::CjsPackageJson,
      4 => ModuleDefFormat::EsmMjs,
      5 => ModuleDefFormat::EsmMts,
      6 => ModuleDefFormat::EsmPackageJson,
      _ => return None,
    }),
    _ => return None,
  })
}

fn write_saved_resolved_id(e: &mut Encoder, value: &SavedResolvedId) {
  write_def_format_from(e, value.module_def_format);
  e.str(value.id.raw_id());
  e.str(value.id.attributes_suffix());
  write_attributes(e, &value.attributes);
  e.u8(match value.external {
    ResolvedExternal::Bool(false) => 0,
    ResolvedExternal::Bool(true) => 1,
    ResolvedExternal::Absolute => 2,
    ResolvedExternal::Relative => 3,
  });
  e.opt(value.normalize_external_id, Encoder::bool);
  e.opt_str(value.package_json_path.as_deref());
  e.opt(value.side_effects, write_hook_side_effects);
  e.bool(value.is_external_without_side_effects);
}

fn read_saved_resolved_id(d: &mut Decoder) -> Option<SavedResolvedId> {
  let module_def_format = read_def_format_from(d)?;
  let raw_id = d.str()?;
  let attributes_suffix = d.str()?;
  Some(SavedResolvedId {
    module_def_format,
    id: ModuleId::from_raw_parts(raw_id, attributes_suffix),
    attributes: read_attributes(d)?,
    external: match d.u8()? {
      0 => ResolvedExternal::Bool(false),
      1 => ResolvedExternal::Bool(true),
      2 => ResolvedExternal::Absolute,
      3 => ResolvedExternal::Relative,
      _ => return None,
    },
    normalize_external_id: d.opt(Decoder::bool)?,
    package_json_path: d.opt_string()?,
    side_effects: d.opt(read_hook_side_effects)?,
    is_external_without_side_effects: d.bool()?,
  })
}

fn write_log_level(e: &mut Encoder, level: LogLevel) {
  e.u8(match level {
    LogLevel::Silent => 0,
    LogLevel::Warn => 1,
    LogLevel::Info => 2,
    LogLevel::Debug => 3,
  });
}

fn read_log_level(d: &mut Decoder) -> Option<LogLevel> {
  Some(match d.u8()? {
    0 => LogLevel::Silent,
    1 => LogLevel::Warn,
    2 => LogLevel::Info,
    3 => LogLevel::Debug,
    _ => return None,
  })
}

fn write_emitted_asset(e: &mut Encoder, asset: &EmittedAsset) {
  e.opt_str(asset.name.as_deref());
  e.opt_str(asset.original_file_name.as_deref());
  e.opt_str(asset.file_name.as_deref());
  write_str_or_bytes(e, &asset.source);
}

fn read_emitted_asset(d: &mut Decoder) -> Option<EmittedAsset> {
  Some(EmittedAsset {
    name: d.opt_string()?,
    original_file_name: d.opt_string()?,
    file_name: d.opt(|d| d.str().map(ArcStr::from))?,
    source: read_str_or_bytes(d)?,
  })
}

fn write_preserve_entry_signatures(e: &mut Encoder, value: PreserveEntrySignatures) {
  e.u8(match value {
    PreserveEntrySignatures::AllowExtension => 0,
    PreserveEntrySignatures::Strict => 1,
    PreserveEntrySignatures::ExportsOnly => 2,
    PreserveEntrySignatures::False => 3,
  });
}

fn read_preserve_entry_signatures(d: &mut Decoder) -> Option<PreserveEntrySignatures> {
  Some(match d.u8()? {
    0 => PreserveEntrySignatures::AllowExtension,
    1 => PreserveEntrySignatures::Strict,
    2 => PreserveEntrySignatures::ExportsOnly,
    3 => PreserveEntrySignatures::False,
    _ => return None,
  })
}

fn write_log(e: &mut Encoder, log: &Log) {
  e.str(&log.message);
  e.opt_str(log.id.as_deref());
  e.opt_str(log.code.as_deref());
  e.opt_str(log.exporter.as_deref());
  e.opt_str(log.binding.as_deref());
  e.opt_str(log.plugin.as_deref());
  e.opt(log.loc.as_ref(), |e, loc| {
    e.u32(loc.line);
    e.u32(loc.column);
    e.opt_str(loc.file.as_deref());
  });
  e.opt(log.pos, Encoder::u32);
  e.opt(log.ids.as_ref(), |e, ids| e.seq(ids.iter(), |e, id| e.str(id)));
}

fn read_log(d: &mut Decoder) -> Option<Log> {
  Some(Log {
    message: d.string()?,
    id: d.opt_string()?,
    code: d.opt_string()?,
    exporter: d.opt_string()?,
    binding: d.opt_string()?,
    plugin: d.opt_string()?,
    loc: d
      .opt(|d| Some(LogLocation { line: d.u32()?, column: d.u32()?, file: d.opt_string()? }))?,
    pos: d.opt(Decoder::u32)?,
    ids: d.opt(|d| d.seq(Decoder::string))?,
    ..Default::default()
  })
}

fn write_action(e: &mut Encoder, action: &RecordedAction) {
  match action {
    RecordedAction::WatchFile(path) => {
      e.u8(0);
      e.str(path);
    }
    RecordedAction::TransformDependency(path) => {
      e.u8(1);
      e.str(path);
    }
    RecordedAction::EmitFile { file, fn_asset_filename, fn_sanitized_file_name } => {
      e.u8(2);
      write_emitted_asset(e, file);
      e.opt_str(fn_asset_filename.as_deref());
      e.opt_str(fn_sanitized_file_name.as_deref());
    }
    RecordedAction::EmitFileAsync(file) => {
      e.u8(3);
      write_emitted_asset(e, file);
    }
    RecordedAction::EmitChunk(chunk) => {
      e.u8(4);
      e.opt_str(chunk.name.as_deref());
      e.opt_str(chunk.file_name.as_deref());
      e.str(&chunk.id);
      e.opt_str(chunk.importer.as_deref());
      e.opt(chunk.preserve_entry_signatures, write_preserve_entry_signatures);
    }
    RecordedAction::EmitPrebuiltChunk(chunk) => {
      e.u8(5);
      e.str(&chunk.file_name);
      e.opt_str(chunk.name.as_deref());
      e.str(&chunk.code);
      e.seq(chunk.exports.iter(), |e, export| e.str(export));
      e.opt(chunk.map.as_ref(), write_sourcemap);
      e.opt_str(chunk.sourcemap_filename.as_deref());
      e.opt_str(chunk.facade_module_id.as_deref());
      e.bool(chunk.is_entry);
      e.bool(chunk.is_dynamic_entry);
    }
    RecordedAction::AssociateModuleWithFileRef { module_id, reference_id } => {
      e.u8(6);
      e.str(module_id);
      e.str(reference_id);
    }
    RecordedAction::Log { level, log } => {
      e.u8(7);
      write_log_level(e, *level);
      write_log(e, log);
    }
    RecordedAction::JsLog { level, json } => {
      e.u8(8);
      write_log_level(e, *level);
      e.str(json);
    }
    RecordedAction::JsModuleMeta { module_id, key, json } => {
      e.u8(9);
      e.str(module_id);
      e.str(key);
      e.opt_str(json.as_deref());
    }
    RecordedAction::JsModuleSideEffects { module_id, value } => {
      e.u8(10);
      e.str(module_id);
      write_hook_side_effects(e, *value);
    }
    RecordedAction::RustModuleMeta { module_id, key, value } => {
      e.u8(13);
      e.str(module_id);
      e.str(key);
      e.bytes(value);
    }
  }
}

fn read_action(d: &mut Decoder) -> Option<RecordedAction> {
  Some(match d.u8()? {
    0 => RecordedAction::WatchFile(d.str()?.into()),
    1 => RecordedAction::TransformDependency(d.str()?.into()),
    2 => RecordedAction::EmitFile {
      file: read_emitted_asset(d)?,
      fn_asset_filename: d.opt_string()?,
      fn_sanitized_file_name: d.opt_string()?,
    },
    3 => RecordedAction::EmitFileAsync(read_emitted_asset(d)?),
    4 => RecordedAction::EmitChunk(EmittedChunk {
      name: d.opt(|d| d.str().map(ArcStr::from))?,
      file_name: d.opt(|d| d.str().map(ArcStr::from))?,
      id: d.string()?,
      importer: d.opt_string()?,
      preserve_entry_signatures: d.opt(read_preserve_entry_signatures)?,
    }),
    5 => RecordedAction::EmitPrebuiltChunk(EmittedPrebuiltChunk {
      file_name: d.str()?.into(),
      name: d.opt(|d| d.str().map(ArcStr::from))?,
      code: d.string()?,
      exports: d.seq(Decoder::string)?,
      map: d.opt(read_sourcemap)?,
      sourcemap_filename: d.opt_string()?,
      facade_module_id: d.opt(|d| d.str().map(ArcStr::from))?,
      is_entry: d.bool()?,
      is_dynamic_entry: d.bool()?,
    }),
    6 => RecordedAction::AssociateModuleWithFileRef {
      module_id: d.str()?.into(),
      reference_id: d.str()?.into(),
    },
    7 => RecordedAction::Log { level: read_log_level(d)?, log: read_log(d)? },
    8 => RecordedAction::JsLog { level: read_log_level(d)?, json: d.string()? },
    9 => RecordedAction::JsModuleMeta {
      module_id: d.str()?.into(),
      key: d.string()?,
      json: d.opt_string()?,
    },
    10 => RecordedAction::JsModuleSideEffects {
      module_id: d.str()?.into(),
      value: read_hook_side_effects(d)?,
    },
    13 => RecordedAction::RustModuleMeta {
      module_id: d.str()?.into(),
      key: d.str()?.into(),
      value: d.bytes()?.into(),
    },
    _ => return None,
  })
}

#[cfg(test)]
mod tests {
  use rolldown_common::{ModuleType, StrOrBytes, side_effects::HookSideEffects};
  use rolldown_plugin::RecordedAction;

  use super::SavedResult;

  #[test]
  fn js_module_options_are_the_same_after_encode_and_decode() {
    let saved = SavedResult {
      source: StrOrBytes::Str("export {}".to_string()),
      module_type: ModuleType::Js,
      hook_side_effects: Some(HookSideEffects::False),
      sourcemap_chain: vec![],
      is_read_from_disk: true,
      resolved: vec![],
      actions: vec![
        RecordedAction::JsModuleMeta {
          module_id: "/src/b.js".into(),
          key: "icons".to_string(),
          json: Some(r#"["home",{"n":1}]"#.to_string()),
        },
        RecordedAction::JsModuleMeta {
          module_id: "/src/b.js".into(),
          key: "cleared".to_string(),
          json: None,
        },
        RecordedAction::JsModuleSideEffects {
          module_id: "/src/a.js".into(),
          value: HookSideEffects::NoTreeshake,
        },
      ],
      resolve_actions: vec![],
    };
    let decoded = SavedResult::decode(&saved.encode()).expect("should decode");
    assert_eq!(format!("{:?}", decoded.actions), format!("{:?}", saved.actions));
    assert_eq!(decoded.hook_side_effects, Some(HookSideEffects::False));
  }

  #[test]
  fn rust_module_meta_is_the_same_after_encode_and_decode() {
    let saved = SavedResult {
      source: StrOrBytes::Str("export {}".to_string()),
      module_type: ModuleType::Js,
      hook_side_effects: None,
      sourcemap_chain: vec![],
      is_read_from_disk: true,
      resolved: vec![],
      actions: vec![RecordedAction::RustModuleMeta {
        module_id: "/src/b.js".into(),
        key: "my-plugin".into(),
        // Not valid UTF-8: a value is bytes, not text.
        value: [0, 255, 1].as_slice().into(),
      }],
      resolve_actions: vec![RecordedAction::RustModuleMeta {
        module_id: "/src/a.js".into(),
        key: "empty".into(),
        value: [].as_slice().into(),
      }],
    };
    let decoded = SavedResult::decode(&saved.encode()).expect("should decode");
    assert_eq!(format!("{:?}", decoded.actions), format!("{:?}", saved.actions));
    assert_eq!(format!("{:?}", decoded.resolve_actions), format!("{:?}", saved.resolve_actions));
  }
}
