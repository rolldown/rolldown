use crate::ast_parser::ExportedSymbol;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct SymbolNode {
    pub symbol: ExportedSymbol,
    pub crate_name: String,
    pub is_entry_crate: bool,
    pub is_secondary_entry: bool,             // 是否是次级入口 crate（被忽略的 crate）
    pub internal_uses: FxHashSet<String>,     // 内部使用的位置
    pub external_uses: FxHashSet<String>,     // 被其他 crate 使用的位置
    pub re_exported_by: FxHashSet<String>,    // 被哪些 crate 重新导出
    pub is_pub_from_entry: bool,              // 是否从入口 crate 公开暴露
}

#[derive(Debug)]
pub struct SymbolGraph {
    nodes: FxHashMap<String, SymbolNode>,     // symbol_key -> SymbolNode
    crate_dependencies: FxHashMap<String, Vec<String>>, // crate -> 它的依赖
    crate_dependents: FxHashMap<String, Vec<String>>,   // crate -> 依赖它的 crates
    pub entry_crate: String,
}

impl SymbolGraph {
    pub fn new(entry_crate: String) -> Self {
        Self {
            nodes: FxHashMap::default(),
            crate_dependencies: FxHashMap::default(),
            crate_dependents: FxHashMap::default(),
            entry_crate,
        }
    }

    pub fn set_dependencies(&mut self, dependencies: FxHashMap<String, Vec<String>>) {
        self.crate_dependencies = dependencies.clone();
        
        // 构建反向依赖图
        for (crate_name, deps) in &dependencies {
            for dep in deps {
                self.crate_dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(crate_name.clone());
            }
        }
    }

    pub fn add_symbol(&mut self, symbol: ExportedSymbol, crate_name: String) {
        let key = format!("{}::{}", crate_name, symbol.name);
        let is_entry_crate = crate_name == self.entry_crate;
        
        self.nodes.insert(
            key,
            SymbolNode {
                symbol,
                crate_name,
                is_entry_crate,
                is_secondary_entry: false,
                internal_uses: FxHashSet::default(),
                external_uses: FxHashSet::default(),
                re_exported_by: FxHashSet::default(),
                is_pub_from_entry: is_entry_crate, // 入口 crate 的符号默认是公开的
            },
        );
    }

    pub fn add_symbol_as_secondary_entry(&mut self, symbol: ExportedSymbol, crate_name: String) {
        let key = format!("{}::{}", crate_name, symbol.name);
        
        self.nodes.insert(
            key,
            SymbolNode {
                symbol: symbol.clone(),
                crate_name,
                is_entry_crate: false,
                is_secondary_entry: true,  // 标记为次级入口
                internal_uses: FxHashSet::default(),
                external_uses: FxHashSet::default(),
                re_exported_by: FxHashSet::default(),
                // 次级入口的公开符号也默认是有用的
                is_pub_from_entry: symbol.is_public,
            },
        );
    }

    pub fn add_internal_use(&mut self, symbol_key: &str, use_location: String) {
        if let Some(node) = self.nodes.get_mut(symbol_key) {
            node.internal_uses.insert(use_location);
        }
    }

    pub fn add_external_use(&mut self, symbol_key: &str, using_crate: String) {
        if let Some(node) = self.nodes.get_mut(symbol_key) {
            node.external_uses.insert(using_crate);
        }
    }

    pub fn add_re_export(&mut self, symbol_key: &str, exporting_crate: String) {
        if let Some(node) = self.nodes.get_mut(symbol_key) {
            node.re_exported_by.insert(exporting_crate.clone());
            
            // 如果被入口 crate 重新导出，标记为从入口公开
            if exporting_crate == self.entry_crate {
                node.is_pub_from_entry = true;
            }
        }
    }

    pub fn propagate_pub_from_entry(&mut self) {
        // 使用 BFS 传播 is_pub_from_entry 标记
        let mut queue = VecDeque::new();
        let mut visited = FxHashSet::default();
        
        // 首先找到所有从入口 crate 重新导出的符号
        for (key, node) in &self.nodes {
            if node.is_pub_from_entry {
                queue.push_back(key.clone());
                visited.insert(key.clone());
            }
        }
        
        // 传播标记
        while let Some(symbol_key) = queue.pop_front() {
            // 收集需要更新的符号
            let mut to_update = Vec::new();
            
            // 如果这个符号被其他符号依赖，继续传播
            if let Some(node) = self.nodes.get(&symbol_key) {
                for re_exporter in &node.re_exported_by {
                    if re_exporter == &self.entry_crate || self.is_reachable_from_entry(re_exporter) {
                        // 收集所有被这个 crate 重新导出的符号
                        for (key, other_node) in &self.nodes {
                            if other_node.re_exported_by.contains(re_exporter) && !visited.contains(key) {
                                to_update.push(key.clone());
                            }
                        }
                    }
                }
            }
            
            // 更新收集到的符号
            for key in to_update {
                if let Some(node) = self.nodes.get_mut(&key) {
                    node.is_pub_from_entry = true;
                    queue.push_back(key.clone());
                    visited.insert(key);
                }
            }
        }
    }

    fn is_reachable_from_entry(&self, crate_name: &str) -> bool {
        // 检查 crate 是否可以从入口 crate 到达
        let mut visited = FxHashSet::default();
        let mut queue = VecDeque::new();
        queue.push_back(self.entry_crate.clone());
        
        while let Some(current) = queue.pop_front() {
            if &current == crate_name {
                return true;
            }
            
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());
            
            if let Some(deps) = self.crate_dependencies.get(&current) {
                for dep in deps {
                    queue.push_back(dep.clone());
                }
            }
        }
        
        false
    }

    pub fn get_crate_symbols(&self, crate_name: &str) -> Vec<String> {
        let mut symbols = Vec::new();
        for (key, node) in &self.nodes {
            if node.crate_name == crate_name {
                symbols.push(key.clone());
            }
        }
        symbols
    }

    pub fn analyze_unused_symbols(&self) -> Vec<UnusedSymbol> {
        let mut unused = Vec::new();
        
        for (key, node) in &self.nodes {
            let is_used = self.is_symbol_used(node);
            
            if !is_used {
                unused.push(UnusedSymbol {
                    symbol_key: key.clone(),
                    symbol: node.symbol.clone(),
                    crate_name: node.crate_name.clone(),
                    reason: self.get_unused_reason(node),
                });
            }
        }
        
        unused
    }

    fn is_symbol_used(&self, node: &SymbolNode) -> bool {
        // 如果有内部使用，认为是有用的
        if !node.internal_uses.is_empty() {
            return true;
        }
        
        // 如果有外部使用，认为是有用的
        if !node.external_uses.is_empty() {
            return true;
        }
        
        // 如果被重新导出，需要进一步检查
        if !node.re_exported_by.is_empty() {
            // 检查重新导出它的 crate 是否被使用或从入口暴露
            for re_exporter in &node.re_exported_by {
                if re_exporter == &self.entry_crate {
                    // 被入口 crate 重新导出，认为是有用的
                    return true;
                }
                
                // 检查重新导出的 crate 是否可从入口到达
                if self.is_reachable_from_entry(re_exporter) {
                    return true;
                }
            }
        }
        
        // 如果是入口 crate 的公开符号，认为是有用的（对外 API）
        if node.is_entry_crate && node.symbol.is_public {
            return true;
        }
        
        // 如果是次级入口 crate（被忽略的 crate）的公开符号，也认为是有用的
        if node.is_secondary_entry && node.symbol.is_public {
            return true;
        }
        
        // 如果从入口 crate 公开暴露，认为是有用的
        if node.is_pub_from_entry {
            return true;
        }
        
        false
    }

    fn get_unused_reason(&self, node: &SymbolNode) -> String {
        if !node.symbol.is_public {
            return "Private symbol not used internally".to_string();
        }
        
        if node.is_entry_crate {
            return "Entry crate symbol not used internally (but exposed as public API)".to_string();
        }
        
        if node.is_secondary_entry {
            return "Secondary entry crate symbol not used internally (treated as public library API)".to_string();
        }
        
        if !self.is_reachable_from_entry(&node.crate_name) {
            return "Symbol in unreachable crate".to_string();
        }
        
        "Public symbol not used by any dependent crate".to_string()
    }

    pub fn get_statistics(&self) -> GraphStatistics {
        let total_symbols = self.nodes.len();
        let mut used_symbols = 0;
        let mut internal_only = 0;
        let mut external_only = 0;
        let mut re_exported = 0;
        let mut pub_from_entry = 0;
        
        for node in self.nodes.values() {
            if self.is_symbol_used(node) {
                used_symbols += 1;
            }
            
            if !node.internal_uses.is_empty() && node.external_uses.is_empty() {
                internal_only += 1;
            }
            
            if node.internal_uses.is_empty() && !node.external_uses.is_empty() {
                external_only += 1;
            }
            
            if !node.re_exported_by.is_empty() {
                re_exported += 1;
            }
            
            if node.is_pub_from_entry {
                pub_from_entry += 1;
            }
        }
        
        let usage_percentage = if total_symbols > 0 {
            (used_symbols as f64 / total_symbols as f64) * 100.0
        } else {
            0.0
        };
        
        GraphStatistics {
            total_symbols,
            used_symbols,
            unused_symbols: total_symbols - used_symbols,
            usage_percentage,
            internal_only,
            external_only,
            re_exported,
            pub_from_entry,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnusedSymbol {
    pub symbol_key: String,
    pub symbol: ExportedSymbol,
    pub crate_name: String,
    pub reason: String,
}

#[derive(Debug)]
pub struct GraphStatistics {
    pub total_symbols: usize,
    pub used_symbols: usize,
    pub unused_symbols: usize,
    pub usage_percentage: f64,
    pub internal_only: usize,
    pub external_only: usize,
    pub re_exported: usize,
    pub pub_from_entry: usize,
}