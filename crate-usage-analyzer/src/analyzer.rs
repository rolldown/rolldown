use crate::ast_parser::{AstParser, CrateSymbols};
use crate::parallel_analyzer::ParallelCrateAnalyzer;
use crate::re_export_detector::ReExportDetector;
use crate::symbol_graph::{GraphStatistics, SymbolGraph, UnusedSymbol};
use crate::trait_impl_tracker::TraitImplTracker;
use crate::workspace_resolver::WorkspaceInfo;
use anyhow::Result;
use colored::*;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::path::PathBuf;

/// Crate usage analyzer with parallel processing
pub struct CrateUsageAnalyzer {
    workspace_info: WorkspaceInfo,
    symbol_graph: SymbolGraph,
    parallel_analyzer: ParallelCrateAnalyzer,
    verbose: bool,
    ignored_crates: FxHashSet<String>,
}

pub struct AnalysisResult {
    pub total_crates: usize,
    pub statistics: GraphStatistics,
    pub unused_symbols: Vec<UnusedSymbol>,
    pub crate_dependencies: FxHashMap<String, Vec<String>>,
}

impl CrateUsageAnalyzer {
    pub fn new(workspace_path: PathBuf, entry_crate: String) -> Result<Self> {
        let workspace_info = WorkspaceInfo::new(&workspace_path)?;
        let symbol_graph = SymbolGraph::new(entry_crate);
        let parallel_analyzer = ParallelCrateAnalyzer::new(false);

        Ok(Self {
            workspace_info,
            symbol_graph,
            parallel_analyzer,
            verbose: false,
            ignored_crates: FxHashSet::default(),
        })
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
        self.parallel_analyzer = ParallelCrateAnalyzer::new(verbose);
    }

    pub fn set_ignored_crates(&mut self, crates: Vec<String>) {
        self.ignored_crates = crates.into_iter().collect();
    }

    pub fn analyze(&mut self) -> Result<AnalysisResult> {
        // 1. Get reachable crates
        let reachable_crates = self.get_reachable_crates();
        
        if self.verbose {
            println!("{}", "🔍 Analyzing workspace...".green().bold());
            println!("  Entry crate: {}", self.symbol_graph.entry_crate.yellow());
            println!("  Reachable crates: {}", reachable_crates.len());
            if !self.ignored_crates.is_empty() {
                println!("  Secondary entry crates: {:?}", self.ignored_crates);
            }
        }

        // 2. Parse symbols from all crates in parallel
        let crate_symbols = self.parse_all_crates_parallel(&reachable_crates)?;
        
        // 3. Build trait trackers for all crates in parallel
        let trait_trackers = self.build_all_trait_trackers(&crate_symbols)?;
        
        // 4. Detect re-exports in parallel
        self.detect_re_exports_parallel(&crate_symbols)?;
        
        // 5. Track internal usage in parallel
        self.track_internal_usage_parallel(&crate_symbols, &trait_trackers)?;
        
        // 6. Track external usage in parallel
        self.track_external_usage_parallel(&crate_symbols, &trait_trackers)?;
        
        // 7. Propagate usage through re-exports
        self.symbol_graph.propagate_pub_from_entry();
        
        // 8. Analyze results
        let unused_symbols = self.symbol_graph.analyze_unused_symbols();
        let statistics = self.symbol_graph.get_statistics();
        
        if self.verbose {
            println!("{}", "✅ Analysis complete!".green().bold());
            println!("  Total symbols: {}", statistics.total_symbols);
            println!("  Used symbols: {}", statistics.used_symbols);
            println!("  Unused symbols: {}", statistics.unused_symbols);
            println!("  Usage rate: {:.1}%", statistics.usage_percentage);
        }
        
        Ok(AnalysisResult {
            total_crates: reachable_crates.len(),
            statistics,
            unused_symbols,
            crate_dependencies: self.workspace_info.dependency_graph.clone(),
        })
    }

    fn parse_all_crates_parallel(&mut self, crate_names: &[String]) -> Result<Vec<(String, CrateSymbols, PathBuf)>> {
        if self.verbose {
            println!("{}", "  📖 Parsing crates in parallel...".cyan());
        }
        
        let results: Vec<_> = crate_names
            .par_iter()
            .filter_map(|crate_name| {
                if let Some(crate_path) = self.workspace_info.get_package_path(crate_name) {
                    let parser = AstParser::new(false);
                    match parser.parse_crate(&crate_path.join("src"), crate_name) {
                        Ok(symbols) => Some((crate_name.clone(), symbols, crate_path)),
                        Err(e) => {
                            if self.verbose {
                                println!("    ⚠️ Failed to parse {}: {}", crate_name, e);
                            }
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .collect();
        
        // Add symbols to graph
        for (crate_name, symbols, _) in &results {
            for symbol in &symbols.exports {
                if self.ignored_crates.contains(crate_name) {
                    self.symbol_graph.add_symbol_as_secondary_entry(symbol.clone(), crate_name.clone());
                } else {
                    self.symbol_graph.add_symbol(symbol.clone(), crate_name.clone());
                }
            }
        }
        
        Ok(results)
    }

    fn build_all_trait_trackers(&mut self, crate_symbols: &[(String, CrateSymbols, PathBuf)]) -> Result<FxHashMap<String, TraitImplTracker>> {
        if self.verbose {
            println!("{}", "  🔧 Building trait trackers...".cyan());
        }
        
        let trackers: Vec<_> = crate_symbols
            .par_iter()
            .map(|(crate_name, _, crate_path)| {
                let analyzer = ParallelCrateAnalyzer::new(false);
                let src_path = crate_path.join("src");
                match analyzer.build_trait_tracker(&src_path) {
                    Ok(tracker) => (crate_name.clone(), tracker),
                    Err(_) => (crate_name.clone(), TraitImplTracker::new()),
                }
            })
            .collect();
        
        Ok(trackers.into_iter().collect())
    }

    fn detect_re_exports_parallel(&mut self, crate_symbols: &[(String, CrateSymbols, PathBuf)]) -> Result<()> {
        if self.verbose {
            println!("{}", "  🔄 Detecting re-exports...".cyan());
        }
        
        let workspace_crates: FxHashSet<String> = crate_symbols.iter().map(|(name, _, _)| name.clone()).collect();
        
        let re_exports: Vec<_> = crate_symbols
            .par_iter()
            .map(|(crate_name, _, crate_path)| {
                let mut detector = ReExportDetector::new();
                let _ = detector.detect_in_crate(&crate_path.join("src"));
                let workspace_re_exports = detector.find_workspace_re_exports(&workspace_crates);
                (crate_name.clone(), workspace_re_exports)
            })
            .collect();
        
        // Apply re-exports to symbol graph
        for (crate_name, crate_re_exports) in re_exports {
            for re_export in crate_re_exports {
                if let Some(from_crate) = &re_export.from_crate {
                    if !re_export.is_glob {
                        let symbol_key = format!("{}::{}", from_crate, re_export.symbol_name);
                        self.symbol_graph.add_re_export(
                            &symbol_key,
                            crate_name.clone()
                        );
                    }
                }
            }
        }
        
        Ok(())
    }

    fn track_internal_usage_parallel(
        &mut self, 
        crate_symbols: &[(String, CrateSymbols, PathBuf)],
        trait_trackers: &FxHashMap<String, TraitImplTracker>
    ) -> Result<()> {
        if self.verbose {
            println!("{}", "  📊 Tracking internal usage...".cyan());
        }
        
        let results: Vec<_> = crate_symbols
            .par_iter()
            .filter_map(|(crate_name, _, crate_path)| {
                let crate_syms = self.symbol_graph.get_crate_symbols(crate_name);
                if crate_syms.is_empty() {
                    return None;
                }
                
                let _tracker = trait_trackers.get(crate_name).cloned().unwrap_or_else(TraitImplTracker::new);
                let analyzer = ParallelCrateAnalyzer::new(false);
                
                match analyzer.analyze_internal_usage(crate_name, &crate_path.join("src"), crate_syms) {
                    Ok(result) => Some(result),
                    Err(_) => None,
                }
            })
            .collect();
        
        // Apply results to symbol graph
        for result in results {
            for (symbol_key, locations) in result.internal_uses {
                for location in locations {
                    self.symbol_graph.add_internal_use(&symbol_key, location);
                }
            }
        }
        
        Ok(())
    }

    fn track_external_usage_parallel(
        &mut self,
        crate_symbols: &[(String, CrateSymbols, PathBuf)],
        trait_trackers: &FxHashMap<String, TraitImplTracker>
    ) -> Result<()> {
        if self.verbose {
            println!("{}", "  📊 Tracking external usage...".cyan());
        }
        
        // Create all pairs of (using_crate, providing_crate)
        let mut pairs = Vec::new();
        for (using_crate, _, using_path) in crate_symbols {
            for (providing_crate, providing_symbols, _) in crate_symbols {
                if using_crate != providing_crate {
                    pairs.push((
                        using_crate.clone(),
                        using_path.clone(),
                        providing_crate.clone(),
                        providing_symbols.exports.clone(),
                    ));
                }
            }
        }
        
        // Process all pairs in parallel
        let results: Vec<_> = pairs
            .par_iter()
            .filter_map(|(using_crate, using_path, providing_crate, provided_symbols)| {
                let tracker = trait_trackers.get(providing_crate).cloned().unwrap_or_else(TraitImplTracker::new);
                let analyzer = ParallelCrateAnalyzer::new(false);
                
                match analyzer.analyze_external_usage(
                    using_crate,
                    &using_path.join("src"),
                    providing_crate,
                    provided_symbols,
                    tracker
                ) {
                    Ok(uses) => Some((using_crate.clone(), uses)),
                    Err(_) => None,
                }
            })
            .collect();
        
        // Apply results to symbol graph
        for (using_crate, uses) in results {
            for (symbol_key, _) in uses {
                self.symbol_graph.add_external_use(&symbol_key, using_crate.clone());
            }
        }
        
        Ok(())
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
}