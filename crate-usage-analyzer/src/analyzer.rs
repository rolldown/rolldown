use crate::ast_parser::{AstParser, CrateSymbols};
use crate::re_export_detector::ReExportDetector;
use crate::symbol_graph::{GraphStatistics, SymbolGraph, UnusedSymbol};
use crate::usage_analyzer::SymbolUsageAnalyzer;
use crate::trait_impl_tracker::TraitImplTracker;
use crate::workspace_resolver::WorkspaceInfo;
use anyhow::{Context, Result};
use colored::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::path::PathBuf;

pub struct CrateUsageAnalyzerV2 {
    workspace_info: WorkspaceInfo,
    ast_parser: AstParser,
    symbol_graph: SymbolGraph,
    verbose: bool,
    ignored_crates: FxHashSet<String>,
}

pub struct AnalysisResultV2 {
    pub total_crates: usize,
    pub statistics: GraphStatistics,
    pub unused_symbols: Vec<UnusedSymbol>,
    pub crate_dependencies: FxHashMap<String, Vec<String>>,
}

impl CrateUsageAnalyzerV2 {
    pub fn new(workspace_path: PathBuf, entry_crate: String) -> Result<Self> {
        let workspace_info = WorkspaceInfo::new(&workspace_path)?;
        let ast_parser = AstParser::new(false);
        let symbol_graph = SymbolGraph::new(entry_crate);

        Ok(Self {
            workspace_info,
            ast_parser,
            symbol_graph,
            verbose: false,
            ignored_crates: FxHashSet::default(),
        })
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    pub fn set_include_private(&mut self, include_private: bool) {
        self.ast_parser = AstParser::new(include_private);
    }

    pub fn set_ignored_crates(&mut self, crate_names: Vec<String>) {
        self.ignored_crates = crate_names.into_iter().collect();
        if self.verbose {
            println!("  ⚠️  Ignoring crates: {:?}", self.ignored_crates);
        }
    }

    pub fn analyze(&mut self) -> Result<AnalysisResultV2> {
        if self.verbose {
            println!("{}", "🔎 Starting comprehensive analysis...".cyan());
        }

        // 1. 设置依赖关系
        self.symbol_graph.set_dependencies(self.workspace_info.dependency_graph.clone());

        // 2. 从入口 crate 开始，使用 BFS 遍历所有可达的 crates
        let reachable_crates = self.get_reachable_crates();
        
        if self.verbose {
            println!("  📦 Found {} reachable crates from entry", reachable_crates.len());
        }

        // 3. 解析所有可达 crate 的符号
        let mut all_crate_symbols = Vec::new();
        for crate_name in &reachable_crates {
            if self.verbose {
                if self.ignored_crates.contains(crate_name) {
                    println!("  📖 Parsing crate (as secondary entry): {}", crate_name.yellow());
                } else {
                    println!("  📖 Parsing crate: {}", crate_name.yellow());
                }
            }

            if let Some(crate_path) = self.workspace_info.get_package_path(crate_name) {
                let symbols = self.ast_parser
                    .parse_crate(&crate_path.join("src"), crate_name)
                    .with_context(|| format!("Failed to parse crate: {}", crate_name))?;

                // 添加符号到图中，如果是被忽略的 crate，标记为次级入口
                for symbol in &symbols.exports {
                    if self.ignored_crates.contains(crate_name) {
                        self.symbol_graph.add_symbol_as_secondary_entry(symbol.clone(), crate_name.clone());
                    } else {
                        self.symbol_graph.add_symbol(symbol.clone(), crate_name.clone());
                    }
                }

                all_crate_symbols.push((crate_name.clone(), symbols, crate_path));
            }
        }

        // 4. 检测每个 crate 的重新导出
        if self.verbose {
            println!("{}", "  🔄 Detecting re-exports...".cyan());
        }

        let workspace_crates: FxHashSet<String> = reachable_crates.iter().cloned().collect();
        
        for (crate_name, _symbols, crate_path) in &all_crate_symbols {
            let mut detector = ReExportDetector::new();
            detector.detect_in_crate(&crate_path.join("src"))?;
            
            let workspace_re_exports = detector.find_workspace_re_exports(&workspace_crates);
            
            for re_export in workspace_re_exports {
                if let Some(from_crate) = &re_export.from_crate {
                    let symbol_key = if re_export.is_glob {
                        // 对于通配符导出，需要特殊处理
                        // 这里简化处理，实际需要展开所有符号
                        continue;
                    } else {
                        format!("{}::{}", from_crate, re_export.symbol_name)
                    };
                    
                    self.symbol_graph.add_re_export(&symbol_key, crate_name.clone());
                    
                    if self.verbose {
                        println!("    ↪️ {} re-exports {} from {}", 
                            crate_name.green(), 
                            re_export.symbol_name.yellow(),
                            from_crate.cyan()
                        );
                    }
                }
            }
        }

        // 5. 分析符号使用情况
        if self.verbose {
            println!("{}", "  📊 Tracking symbol usage...".cyan());
        }

        self.track_symbol_usage(&all_crate_symbols)?;

        // 6. 分析内部使用
        for (crate_name, _symbols, crate_path) in &all_crate_symbols {
            self.track_internal_usage(crate_name, &crate_path.join("src"))?;
        }

        // 7. 传播 pub_from_entry 标记
        self.symbol_graph.propagate_pub_from_entry();

        // 8. 分析未使用的符号
        let unused_symbols = self.symbol_graph.analyze_unused_symbols();
        let statistics = self.symbol_graph.get_statistics();

        if self.verbose {
            println!("{}", "✅ Analysis complete!".green().bold());
            println!("  Total symbols: {}", statistics.total_symbols);
            println!("  Used symbols: {}", statistics.used_symbols);
            println!("  Unused symbols: {}", statistics.unused_symbols);
            println!("  Re-exported symbols: {}", statistics.re_exported);
            println!("  Public from entry: {}", statistics.pub_from_entry);
        }

        Ok(AnalysisResultV2 {
            total_crates: reachable_crates.len(),
            statistics,
            unused_symbols,
            crate_dependencies: self.workspace_info.dependency_graph.clone(),
        })
    }

    fn get_reachable_crates(&self) -> Vec<String> {
        let mut reachable = Vec::new();
        let mut visited = FxHashSet::default();
        let mut queue = VecDeque::new();
        
        let entry = self.symbol_graph.entry_crate.clone();
        queue.push_back(entry.clone());
        
        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) {
                continue;
            }
            
            visited.insert(current.clone());
            reachable.push(current.clone());
            
            if let Some(deps) = self.workspace_info.dependency_graph.get(&current) {
                for dep in deps {
                    if !visited.contains(dep) {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }
        
        reachable
    }

    fn track_symbol_usage(&mut self, all_crate_symbols: &[(String, CrateSymbols, PathBuf)]) -> Result<()> {
        // 对每个 crate，检查它使用了哪些其他 crate 的符号
        for (using_crate, _symbols, crate_path) in all_crate_symbols {
            for (providing_crate, providing_symbols, _) in all_crate_symbols {
                if using_crate == providing_crate {
                    continue; // 跳过自己
                }
                
                // 检查 using_crate 是否使用了 providing_crate 的符号
                self.track_usage_between_crates(
                    using_crate,
                    &crate_path.join("src"),
                    providing_crate,
                    &providing_symbols.exports
                )?;
            }
        }
        
        Ok(())
    }

    fn track_usage_between_crates(
        &mut self,
        using_crate: &str,
        using_crate_path: &PathBuf,
        providing_crate: &str,
        provided_symbols: &[crate::ast_parser::ExportedSymbol],
    ) -> Result<()> {
        use walkdir::WalkDir;
        
        // 首先构建 providing_crate 的 trait 实现追踪器
        // 需要获取 providing_crate 的路径
        let mut trait_tracker = TraitImplTracker::new();
        if let Some(providing_crate_path) = self.workspace_info.get_package_path(providing_crate) {
            let src_path = providing_crate_path.join("src");
            for entry in WalkDir::new(&src_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
            {
                let file_path = entry.path();
                if let Err(_) = trait_tracker.analyze_file(&file_path.to_path_buf()) {
                    // Silently skip files that fail to parse
                }
            }
        }
        
        // 创建 AST 分析器并设置 trait 追踪器
        let mut analyzer = SymbolUsageAnalyzer::new(using_crate.to_string());
        analyzer.set_trait_tracker(trait_tracker);
        
        // 添加要追踪的符号
        for symbol in provided_symbols {
            analyzer.add_tracked_symbol(symbol.name.clone(), providing_crate.to_string());
        }
        
        // 分析每个源文件
        for entry in WalkDir::new(using_crate_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        {
            let file_path = entry.path();
            let content = std::fs::read_to_string(file_path)?;
            
            // 使用 AST 分析
            if let Err(e) = analyzer.analyze_file(file_path, &content) {
                if self.verbose {
                    println!("    ⚠️  Failed to parse {}: {}", file_path.display(), e);
                }
                // AST 解析失败时跳过该文件
                continue;
            }
        }
        
        // 记录检测到的外部使用
        for (symbol_key, _locations) in analyzer.get_detected_uses() {
            self.symbol_graph.add_external_use(symbol_key, using_crate.to_string());
        }
        
        Ok(())
    }

    fn track_internal_usage(&mut self, crate_name: &str, crate_path: &PathBuf) -> Result<()> {
        use walkdir::WalkDir;
        
        // 获取该 crate 定义的所有符号
        let crate_symbols = self.get_crate_symbols(crate_name);
        if crate_symbols.is_empty() {
            return Ok(());
        }
        
        // 首先构建 trait 实现追踪器
        let mut trait_tracker = TraitImplTracker::new();
        
        // 扫描所有文件以构建 trait 实现映射
        for entry in WalkDir::new(crate_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        {
            let file_path = entry.path();
            if let Err(e) = trait_tracker.analyze_file(&file_path.to_path_buf()) {
                if self.verbose {
                    println!("    ⚠️  Failed to analyze trait impls in {}: {}", file_path.display(), e);
                }
            }
        }
        
        // 创建 AST 分析器并设置 trait 追踪器
        let mut analyzer = SymbolUsageAnalyzer::new(crate_name.to_string());
        analyzer.set_trait_tracker(trait_tracker);
        
        // 添加要追踪的符号
        for symbol_key in &crate_symbols {
            if let Some(symbol_name) = symbol_key.split("::").last() {
                analyzer.add_tracked_symbol(symbol_name.to_string(), crate_name.to_string());
            }
        }
        
        // 分析每个源文件
        for entry in WalkDir::new(crate_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        {
            let file_path = entry.path();
            let content = std::fs::read_to_string(file_path)?;
            
            // 使用 AST 分析
            if let Err(e) = analyzer.analyze_file(file_path, &content) {
                if self.verbose {
                    println!("    ⚠️  Failed to parse {}: {}", file_path.display(), e);
                }
                // AST 解析失败时跳过该文件
                continue;
            }
        }
        
        // 记录检测到的内部使用
        for (symbol_key, locations) in analyzer.get_detected_uses() {
            for location in locations {
                self.symbol_graph.add_internal_use(symbol_key, location.clone());
                
                if self.verbose && symbol_key.ends_with("::try_from_path") {
                    println!("    ✅ Found internal use of try_from_path in {}", location);
                }
            }
        }
        
        Ok(())
    }

    fn get_crate_symbols(&self, crate_name: &str) -> Vec<String> {
        // 从 symbol_graph 中获取属于指定 crate 的符号
        self.symbol_graph.get_crate_symbols(crate_name)
    }
}