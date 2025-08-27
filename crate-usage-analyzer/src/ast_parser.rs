use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{Item, ItemUse, Visibility};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ExportedSymbol {
  pub name: String,
  pub kind: SymbolKind,
  pub file_path: PathBuf,
  pub line: usize,
  pub column: usize,
  pub is_public: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
  Function,
  Struct,
  Enum,
  Trait,
  Type,
  Const,
  Static,
}

pub struct CrateSymbols {
  pub exports: Vec<ExportedSymbol>,
}

pub struct AstParser {
  include_private: bool,
}

impl AstParser {
  pub fn new(include_private: bool) -> Self {
    Self { include_private }
  }

  pub fn parse_crate(&self, crate_path: &Path, _crate_name: &str) -> Result<CrateSymbols> {
    let mut exports = Vec::new();

    for entry in WalkDir::new(crate_path)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
      let file_path = entry.path();
      let _relative_path = file_path.strip_prefix(crate_path).unwrap_or(file_path);

      let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {:?}", file_path))?;

      let ast = syn::parse_file(&content)
        .with_context(|| format!("Failed to parse file: {:?}", file_path))?;

      let mut visitor = SymbolExtractor {
        exports: &mut exports,
        file_path: file_path.to_path_buf(),
        include_private: self.include_private,
        source_code: content.clone(),
      };

      visitor.visit_file(&ast);
    }

    Ok(CrateSymbols { exports })
  }
}

struct SymbolExtractor<'a> {
  exports: &'a mut Vec<ExportedSymbol>,
  file_path: PathBuf,
  include_private: bool,
  source_code: String,
}

impl<'a> SymbolExtractor<'a> {
  fn find_symbol_location(&self, symbol_name: &str, kind: &SymbolKind) -> (usize, usize) {
    // 根据符号类型构建搜索模式
    let patterns = match kind {
      SymbolKind::Function => vec![
        format!("fn {}", symbol_name),
        format!("pub fn {}", symbol_name),
        format!("pub(crate) fn {}", symbol_name),
        format!("pub(super) fn {}", symbol_name),
        format!("async fn {}", symbol_name),
        format!("pub async fn {}", symbol_name),
        format!("const fn {}", symbol_name),
        format!("pub const fn {}", symbol_name),
        format!("unsafe fn {}", symbol_name),
        format!("pub unsafe fn {}", symbol_name),
      ],
      SymbolKind::Struct => vec![
        format!("struct {}", symbol_name),
        format!("pub struct {}", symbol_name),
        format!("pub(crate) struct {}", symbol_name),
        format!("pub(super) struct {}", symbol_name),
      ],
      SymbolKind::Enum => vec![
        format!("enum {}", symbol_name),
        format!("pub enum {}", symbol_name),
        format!("pub(crate) enum {}", symbol_name),
        format!("pub(super) enum {}", symbol_name),
      ],
      SymbolKind::Trait => vec![
        format!("trait {}", symbol_name),
        format!("pub trait {}", symbol_name),
        format!("pub(crate) trait {}", symbol_name),
        format!("pub(super) trait {}", symbol_name),
      ],
      SymbolKind::Type => vec![
        format!("type {}", symbol_name),
        format!("pub type {}", symbol_name),
        format!("pub(crate) type {}", symbol_name),
        format!("pub(super) type {}", symbol_name),
      ],
      SymbolKind::Const => vec![
        format!("const {}", symbol_name),
        format!("pub const {}", symbol_name),
        format!("pub(crate) const {}", symbol_name),
        format!("pub(super) const {}", symbol_name),
      ],
      SymbolKind::Static => vec![
        format!("static {}", symbol_name),
        format!("pub static {}", symbol_name),
        format!("pub(crate) static {}", symbol_name),
        format!("pub(super) static {}", symbol_name),
      ],
    };

    // 在源代码中搜索这些模式
    for pattern in &patterns {
      if let Some(pos) = self.source_code.find(pattern) {
        return self.byte_offset_to_line_column(pos);
      }
    }

    // 如果找不到，尝试只搜索符号名（作为后备）
    if let Some(pos) = self.source_code.find(symbol_name) {
      return self.byte_offset_to_line_column(pos);
    }

    // 默认返回 1:1
    (1, 1)
  }

  fn byte_offset_to_line_column(&self, byte_offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut last_newline = 0;

    for (i, ch) in self.source_code.char_indices() {
      if i >= byte_offset {
        break;
      }
      if ch == '\n' {
        line += 1;
        last_newline = i + 1;
      }
    }

    let column = byte_offset - last_newline + 1;
    (line, column)
  }

  fn create_symbol(&self, name: String, kind: SymbolKind, is_public: bool) -> ExportedSymbol {
    let (line, column) = self.find_symbol_location(&name, &kind);
    ExportedSymbol { name, kind, file_path: self.file_path.clone(), line, column, is_public }
  }
}

impl<'a> Visit<'_> for SymbolExtractor<'a> {
  fn visit_item(&mut self, item: &Item) {
    let is_public = matches!(&item, Item::Mod(m) if matches!(m.vis, Visibility::Public(_)))
      || matches!(&item, Item::Struct(s) if matches!(s.vis, Visibility::Public(_)))
      || matches!(&item, Item::Enum(e) if matches!(e.vis, Visibility::Public(_)))
      || matches!(&item, Item::Fn(f) if matches!(f.vis, Visibility::Public(_)))
      || matches!(&item, Item::Trait(t) if matches!(t.vis, Visibility::Public(_)))
      || matches!(&item, Item::Type(t) if matches!(t.vis, Visibility::Public(_)))
      || matches!(&item, Item::Const(c) if matches!(c.vis, Visibility::Public(_)))
      || matches!(&item, Item::Static(s) if matches!(s.vis, Visibility::Public(_)))
      || matches!(&item, Item::Macro(m) if m.mac.path.segments.first().map_or(false, |s| s.ident == "macro_rules"));

    if !is_public && !self.include_private {
      visit::visit_item(self, item);
      return;
    }

    match item {
      Item::Fn(func) => {
        self.exports.push(self.create_symbol(
          func.sig.ident.to_string(),
          SymbolKind::Function,
          is_public,
        ));
      }
      Item::Struct(s) => {
        self.exports.push(self.create_symbol(s.ident.to_string(), SymbolKind::Struct, is_public));
      }
      Item::Enum(e) => {
        self.exports.push(self.create_symbol(e.ident.to_string(), SymbolKind::Enum, is_public));
      }
      Item::Trait(t) => {
        self.exports.push(self.create_symbol(t.ident.to_string(), SymbolKind::Trait, is_public));
      }
      Item::Type(t) => {
        self.exports.push(self.create_symbol(t.ident.to_string(), SymbolKind::Type, is_public));
      }
      Item::Const(c) => {
        self.exports.push(self.create_symbol(c.ident.to_string(), SymbolKind::Const, is_public));
      }
      Item::Static(s) => {
        self.exports.push(self.create_symbol(s.ident.to_string(), SymbolKind::Static, is_public));
      }
      Item::Use(_) => {}
      _ => {}
    }

    visit::visit_item(self, item);
  }

  fn visit_item_use(&mut self, node: &ItemUse) {
    visit::visit_item_use(self, node);
  }
}
