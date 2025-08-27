use anyhow::Result;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

use crate::ast_parser::ExportedSymbol;
use crate::trait_impl_tracker::TraitImplTracker;
use crate::usage_analyzer::SymbolUsageAnalyzer;

/// Parallel analysis results for a crate
pub struct CrateAnalysisResult {
    pub crate_name: String,
    pub internal_uses: FxHashMap<String, FxHashSet<String>>,
    pub external_uses: FxHashMap<String, FxHashSet<String>>,
    pub trait_tracker: TraitImplTracker,
}

/// Analyzes a crate in parallel
pub struct ParallelCrateAnalyzer {
    verbose: bool,
}

impl ParallelCrateAnalyzer {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
    
    /// Analyze a crate's internal usage in parallel
    pub fn analyze_internal_usage(
        &self,
        crate_name: &str,
        crate_path: &Path,
        crate_symbols: Vec<String>,
    ) -> Result<CrateAnalysisResult> {
        // First pass: build trait tracker in parallel
        let trait_tracker = self.build_trait_tracker(crate_path)?;
        
        // Collect all Rust files
        let rust_files: Vec<PathBuf> = WalkDir::new(crate_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
            .map(|e| e.path().to_path_buf())
            .collect();
        
        // Second pass: analyze usage in parallel
        let internal_uses = Arc::new(Mutex::new(FxHashMap::default()));
        
        rust_files.par_iter().for_each(|file_path| {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                let mut analyzer = SymbolUsageAnalyzer::new(crate_name.to_string());
                analyzer.set_trait_tracker(trait_tracker.clone());
                
                // Add symbols to track
                for symbol_key in &crate_symbols {
                    if let Some(symbol_name) = symbol_key.split("::").last() {
                        analyzer.add_tracked_symbol(symbol_name.to_string(), crate_name.to_string());
                    }
                }
                
                // Analyze file
                if let Ok(_) = analyzer.analyze_file(file_path, &content) {
                    // Merge results
                    let detected_uses = analyzer.get_detected_uses();
                    let mut uses = internal_uses.lock().unwrap();
                    for (symbol_key, locations) in detected_uses {
                        uses.entry(symbol_key.clone())
                            .or_insert_with(FxHashSet::default)
                            .extend(locations.clone());
                    }
                }
            }
        });
        
        Ok(CrateAnalysisResult {
            crate_name: crate_name.to_string(),
            internal_uses: Arc::try_unwrap(internal_uses).unwrap().into_inner().unwrap(),
            external_uses: FxHashMap::default(),
            trait_tracker,
        })
    }
    
    /// Analyze usage between crates in parallel
    pub fn analyze_external_usage(
        &self,
        using_crate: &str,
        using_crate_path: &Path,
        providing_crate: &str,
        provided_symbols: &[ExportedSymbol],
        trait_tracker: TraitImplTracker,
    ) -> Result<FxHashMap<String, FxHashSet<String>>> {
        // Collect all Rust files
        let rust_files: Vec<PathBuf> = WalkDir::new(using_crate_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
            .map(|e| e.path().to_path_buf())
            .collect();
        
        let external_uses = Arc::new(Mutex::new(FxHashMap::default()));
        
        rust_files.par_iter().for_each(|file_path| {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                let mut analyzer = SymbolUsageAnalyzer::new(using_crate.to_string());
                analyzer.set_trait_tracker(trait_tracker.clone());
                
                // Add symbols to track
                for symbol in provided_symbols {
                    analyzer.add_tracked_symbol(symbol.name.clone(), providing_crate.to_string());
                }
                
                // Analyze file
                if let Ok(_) = analyzer.analyze_file(file_path, &content) {
                    // Merge results
                    let detected_uses = analyzer.get_detected_uses();
                    let mut uses = external_uses.lock().unwrap();
                    for (symbol_key, locations) in detected_uses {
                        uses.entry(symbol_key.clone())
                            .or_insert_with(FxHashSet::default)
                            .extend(locations.clone());
                    }
                }
            }
        });
        
        Ok(Arc::try_unwrap(external_uses).unwrap().into_inner().unwrap())
    }
    
    /// Build trait tracker in parallel
    pub fn build_trait_tracker(&self, crate_path: &Path) -> Result<TraitImplTracker> {
        let rust_files: Vec<PathBuf> = WalkDir::new(crate_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
            .map(|e| e.path().to_path_buf())
            .collect();
        
        // Parse trait impls in parallel and collect results
        let trait_impls: Vec<_> = rust_files
            .par_iter()
            .filter_map(|file_path| {
                let mut local_tracker = TraitImplTracker::new();
                if let Ok(_) = local_tracker.analyze_file(file_path) {
                    Some(local_tracker)
                } else {
                    None
                }
            })
            .collect();
        
        // Merge all trackers
        let mut merged_tracker = TraitImplTracker::new();
        for tracker in trait_impls {
            merged_tracker.merge(tracker);
        }
        
        Ok(merged_tracker)
    }
}