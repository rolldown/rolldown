use anyhow::{Context, Result};
use rustc_hash::FxHashSet;
use std::path::Path;
use syn::visit::{self, Visit};
use syn::{Item, ItemUse, UseTree, Visibility};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ReExport {
  pub from_crate: Option<String>, // 来源 crate（如果是外部的）
  pub symbol_name: String,        // 符号名称
  pub is_glob: bool,              // 是否是通配符导出 (*)
}

pub struct ReExportDetector {
  pub re_exports: Vec<ReExport>,
}

impl ReExportDetector {
  pub fn new() -> Self {
    Self { re_exports: Vec::new() }
  }

  pub fn detect_in_crate(&mut self, crate_path: &Path) -> Result<()> {
    for entry in WalkDir::new(crate_path)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
      let file_path = entry.path();
      let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {:?}", file_path))?;

      let ast = syn::parse_file(&content)
        .with_context(|| format!("Failed to parse file: {:?}", file_path))?;

      let mut visitor = ReExportVisitor { re_exports: &mut self.re_exports };

      visitor.visit_file(&ast);
    }

    Ok(())
  }

  pub fn find_workspace_re_exports(&self, workspace_crates: &FxHashSet<String>) -> Vec<ReExport> {
    self
      .re_exports
      .iter()
      .filter(|re| re.from_crate.as_ref().map_or(false, |c| workspace_crates.contains(c)))
      .cloned()
      .collect()
  }
}

struct ReExportVisitor<'a> {
  re_exports: &'a mut Vec<ReExport>,
}

impl<'a> Visit<'_> for ReExportVisitor<'a> {
  fn visit_item(&mut self, item: &Item) {
    if let Item::Use(use_item) = item {
      // 只处理 pub use（重新导出）
      if matches!(use_item.vis, Visibility::Public(_)) {
        self.process_pub_use(&use_item);
      }
    }

    // 继续访问嵌套的 mod
    visit::visit_item(self, item);
  }
}

impl<'a> ReExportVisitor<'a> {
  fn process_pub_use(&mut self, use_item: &ItemUse) {
    self.process_use_tree(&use_item.tree, Vec::new());
  }

  fn process_use_tree(&mut self, tree: &UseTree, mut path: Vec<String>) {
    match tree {
      UseTree::Path(p) => {
        path.push(p.ident.to_string());
        self.process_use_tree(&p.tree, path);
      }
      UseTree::Name(n) => {
        let symbol_name = n.ident.to_string();
        let from_crate = path.first().cloned();

        self.re_exports.push(ReExport { from_crate, symbol_name, is_glob: false });
      }
      UseTree::Rename(r) => {
        let _symbol_name = r.ident.to_string();
        let as_name = r.rename.to_string();
        let from_crate = path.first().cloned();

        self.re_exports.push(ReExport { from_crate, symbol_name: as_name, is_glob: false });
      }
      UseTree::Glob(_) => {
        let from_crate = path.first().cloned();

        self.re_exports.push(ReExport { from_crate, symbol_name: "*".to_string(), is_glob: true });
      }
      UseTree::Group(g) => {
        for item in &g.items {
          self.process_use_tree(item, path.clone());
        }
      }
    }
  }
}
