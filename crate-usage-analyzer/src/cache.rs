use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Cache for parsed file ASTs and analysis results
pub struct AnalysisCache {
    /// Cached parsed ASTs (file_path -> (last_modified, ast))
    ast_cache: FxHashMap<PathBuf, (SystemTime, syn::File)>,
    
    /// Cached trait implementations per crate
    trait_impl_cache: FxHashMap<String, crate::trait_impl_tracker::TraitImplTracker>,
}

impl AnalysisCache {
    pub fn new() -> Self {
        Self {
            ast_cache: FxHashMap::default(),
            trait_impl_cache: FxHashMap::default(),
        }
    }
    
    /// Get or parse a file's AST
    pub fn get_or_parse_file(&mut self, path: &Path) -> Result<&syn::File, Box<dyn std::error::Error>> {
        let metadata = std::fs::metadata(path)?;
        let modified = metadata.modified()?;
        
        // Check if we have a cached version
        if let Some((cached_time, _)) = self.ast_cache.get(path) {
            if *cached_time == modified {
                return Ok(&self.ast_cache.get(path).unwrap().1);
            }
        }
        
        // Parse the file
        let content = std::fs::read_to_string(path)?;
        let ast = syn::parse_file(&content)?;
        
        // Cache it
        self.ast_cache.insert(path.to_path_buf(), (modified, ast));
        Ok(&self.ast_cache.get(path).unwrap().1)
    }
    
    /// Get cached trait tracker for a crate
    pub fn get_trait_tracker(&self, crate_name: &str) -> Option<&crate::trait_impl_tracker::TraitImplTracker> {
        self.trait_impl_cache.get(crate_name)
    }
    
    /// Cache trait tracker for a crate
    pub fn cache_trait_tracker(&mut self, crate_name: String, tracker: crate::trait_impl_tracker::TraitImplTracker) {
        self.trait_impl_cache.insert(crate_name, tracker);
    }
}