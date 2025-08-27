use crate::trait_impl_tracker::TraitImplTracker;
use anyhow::Result;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::Path;
use syn::visit::Visit;
use syn::{
  Expr, ExprField, ExprMethodCall, ExprPath, ItemUse, Pat, PatStruct, PatTupleStruct,
  Path as SynPath, Stmt, Type, TypePath, UseTree,
};

/// 符号使用分析器 - 使用 AST 分析检测符号使用
pub struct SymbolUsageAnalyzer {
  /// 要追踪的符号及其所属 crate
  tracked_symbols: FxHashMap<String, String>,
  /// 检测到的使用 (symbol_key -> locations)
  detected_uses: FxHashMap<String, FxHashSet<String>>,
  /// 当前文件路径
  current_file: String,
  /// use 导入的映射 (alias -> full_path)
  use_imports: FxHashMap<String, String>,
  /// Trait 实现追踪器
  trait_tracker: Option<TraitImplTracker>,
}

impl SymbolUsageAnalyzer {
  pub fn new() -> Self {
    Self {
      tracked_symbols: FxHashMap::default(),
      detected_uses: FxHashMap::default(),
      current_file: String::new(),
      use_imports: FxHashMap::default(),
      trait_tracker: None,
    }
  }

  /// Set the trait tracker for this analyzer
  pub fn set_trait_tracker(&mut self, tracker: TraitImplTracker) {
    self.trait_tracker = Some(tracker);
  }

  /// 添加要追踪的符号
  pub fn add_tracked_symbol(&mut self, symbol_name: String, crate_name: String) {
    self.tracked_symbols.insert(symbol_name, crate_name);
  }

  /// 分析文件中的符号使用
  pub fn analyze_file(&mut self, file_path: &Path, content: &str) -> Result<()> {
    self.current_file = file_path.display().to_string();
    self.use_imports.clear();

    // 解析文件
    let ast = match syn::parse_file(content) {
      Ok(ast) => ast,
      Err(e) => {
        // AST 解析失败，返回错误但不中断整体分析
        return Err(anyhow::anyhow!("Failed to parse {}: {}", file_path.display(), e));
      }
    };

    // 首先收集所有的 use 语句
    self.collect_use_statements(&ast);

    // 然后分析符号使用
    self.visit_file(&ast);

    Ok(())
  }

  /// 获取检测到的使用
  pub fn get_detected_uses(&self) -> &FxHashMap<String, FxHashSet<String>> {
    &self.detected_uses
  }

  fn collect_use_statements(&mut self, file: &syn::File) {
    for item in &file.items {
      if let syn::Item::Use(use_item) = item {
        self.process_use_tree(&use_item.tree, Vec::new());
      }
    }
  }

  fn process_use_tree(&mut self, tree: &UseTree, mut prefix: Vec<String>) {
    match tree {
      UseTree::Path(use_path) => {
        prefix.push(use_path.ident.to_string());
        self.process_use_tree(&use_path.tree, prefix);
      }
      UseTree::Name(use_name) => {
        prefix.push(use_name.ident.to_string());
        let full_path = prefix.join("::");
        let local_name = use_name.ident.to_string();
        self.use_imports.insert(local_name, full_path);
      }
      UseTree::Rename(use_rename) => {
        prefix.push(use_rename.ident.to_string());
        let full_path = prefix.join("::");
        let alias = use_rename.rename.to_string();
        self.use_imports.insert(alias, full_path);
      }
      UseTree::Group(use_group) => {
        for item in &use_group.items {
          self.process_use_tree(item, prefix.clone());
        }
      }
      UseTree::Glob(_) => {
        // 通配符导入 - 记录前缀
        if !prefix.is_empty() {
          let prefix_path = prefix.join("::");
          self.use_imports.insert(format!("{}::*", prefix_path), prefix_path);
        }
      }
    }
  }

  fn record_potential_use(&mut self, ident: &str, full_path: Option<String>) {
    // 检查是否是我们追踪的符号
    if let Some(crate_name) = self.tracked_symbols.get(ident) {
      let symbol_key = format!("{}::{}", crate_name, ident);
      self.detected_uses.entry(symbol_key).or_default().insert(self.current_file.clone());
      return;
    }

    // 如果有完整路径，检查路径中的最后一个段
    if let Some(path) = full_path {
      if let Some(last_segment) = path.split("::").last() {
        if let Some(crate_name) = self.tracked_symbols.get(last_segment) {
          let symbol_key = format!("{}::{}", crate_name, last_segment);
          self.detected_uses.entry(symbol_key).or_default().insert(self.current_file.clone());
        }
      }
    }

    // 检查是否通过 use 导入
    if let Some(imported_path) = self.use_imports.get(ident) {
      if let Some(last_segment) = imported_path.split("::").last() {
        if let Some(crate_name) = self.tracked_symbols.get(last_segment) {
          let symbol_key = format!("{}::{}", crate_name, last_segment);
          self.detected_uses.entry(symbol_key).or_default().insert(self.current_file.clone());
        }
      }
    }
  }

  fn extract_path_string(path: &SynPath) -> String {
    path.segments.iter().map(|seg| seg.ident.to_string()).collect::<Vec<_>>().join("::")
  }
}

impl<'ast> Visit<'ast> for SymbolUsageAnalyzer {
  fn visit_expr_path(&mut self, node: &'ast ExprPath) {
    let path_str = Self::extract_path_string(&node.path);

    // 检查完整路径
    if path_str.contains("::") {
      // 例如: module::function 或 crate::module::function
      if let Some(last_segment) = path_str.split("::").last() {
        self.record_potential_use(last_segment, Some(path_str.clone()));

        // Check if this is an associated function from an extension trait
        // e.g., FxHashSet::with_capacity where with_capacity is from FxHashSetExt
        let trait_names: Vec<String> = if let Some(ref tracker) = self.trait_tracker {
          if let Some(traits) = tracker.get_traits_for_method(last_segment) {
            traits.iter().cloned().collect()
          } else {
            Vec::new()
          }
        } else {
          Vec::new()
        };

        // Record usage of each trait
        for trait_name in trait_names {
          self.record_potential_use(&trait_name, None);
        }
      }
    } else {
      // 单个标识符
      self.record_potential_use(&path_str, None);
    }

    syn::visit::visit_expr_path(self, node);
  }

  fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
    let method_name = node.method.to_string();

    // First check if this method is tracked directly
    self.record_potential_use(&method_name, None);

    // Then check if this method belongs to an extension trait
    let trait_names: Vec<String> = if let Some(ref tracker) = self.trait_tracker {
      if let Some(traits) = tracker.get_traits_for_method(&method_name) {
        // Collect trait names to avoid borrow issues
        traits.iter().cloned().collect()
      } else {
        Vec::new()
      }
    } else {
      Vec::new()
    };

    // Record usage of each trait
    for trait_name in trait_names {
      self.record_potential_use(&trait_name, None);
    }

    syn::visit::visit_expr_method_call(self, node);
  }

  fn visit_expr_field(&mut self, node: &'ast ExprField) {
    if let syn::Member::Named(ref ident) = node.member {
      let field_name = ident.to_string();
      self.record_potential_use(&field_name, None);
    }
    syn::visit::visit_expr_field(self, node);
  }

  fn visit_type_path(&mut self, node: &'ast TypePath) {
    let path_str = Self::extract_path_string(&node.path);

    if path_str.contains("::") {
      if let Some(last_segment) = path_str.split("::").last() {
        self.record_potential_use(last_segment, Some(path_str.clone()));
      }
    } else {
      self.record_potential_use(&path_str, None);
    }

    syn::visit::visit_type_path(self, node);
  }

  fn visit_pat_struct(&mut self, node: &'ast PatStruct) {
    let path_str = Self::extract_path_string(&node.path);

    if let Some(last_segment) = path_str.split("::").last() {
      self.record_potential_use(last_segment, Some(path_str.clone()));
    }

    syn::visit::visit_pat_struct(self, node);
  }

  fn visit_pat_tuple_struct(&mut self, node: &'ast PatTupleStruct) {
    let path_str = Self::extract_path_string(&node.path);

    if let Some(last_segment) = path_str.split("::").last() {
      self.record_potential_use(last_segment, Some(path_str.clone()));
    }

    syn::visit::visit_pat_tuple_struct(self, node);
  }

  fn visit_item_use(&mut self, node: &'ast ItemUse) {
    // use 语句已在 collect_use_statements 中处理
    syn::visit::visit_item_use(self, node);
  }

  fn visit_expr(&mut self, node: &'ast Expr) {
    // 处理其他表达式类型
    syn::visit::visit_expr(self, node);
  }

  fn visit_stmt(&mut self, node: &'ast Stmt) {
    // 处理语句
    syn::visit::visit_stmt(self, node);
  }

  fn visit_type(&mut self, node: &'ast Type) {
    // 处理类型
    syn::visit::visit_type(self, node);
  }

  fn visit_pat(&mut self, node: &'ast Pat) {
    // 处理模式
    syn::visit::visit_pat(self, node);
  }
}
