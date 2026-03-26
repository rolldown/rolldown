use std::{
  collections::HashMap,
  ffi::OsString,
  fmt, io,
  path::{Path, PathBuf},
  sync::Arc,
};

use dashmap::DashMap;
use oxc_resolver::{FileMetadata, FileSystem as OxcResolverFileSystem, FileSystemOs, ResolveError};

use crate::file_system::FileSystem;

/// Cached directory entry: file type info from readdir's d_type
#[derive(Debug, Clone, Copy)]
struct DirEntryInfo {
  is_file: bool,
  is_dir: bool,
  is_symlink: bool,
}

/// Result of reading a directory: Some(entries) or None (not a dir / doesn't exist)
type DirEntries = Option<Arc<HashMap<OsString, DirEntryInfo>>>;

/// Operating System filesystem with directory entry caching.
///
/// When `metadata()` is called for a path, instead of making a single `statx` syscall,
/// we read all entries in the parent directory via `readdir` (which uses `getdents64` on
/// Linux and gets `d_type` for free — no extra syscalls). Subsequent lookups in the
/// same directory are pure in-memory HashMap lookups.
#[derive(Clone)]
pub struct OsFileSystem {
  inner: Arc<FileSystemOs>,
  /// Cache: directory path -> directory entries (Some = entries, None = not a dir)
  dir_cache: Arc<DashMap<PathBuf, DirEntries>>,
}

impl fmt::Debug for OsFileSystem {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "OsFileSystem")
  }
}

impl OsFileSystem {
  /// Read a directory from disk and return its entries.
  fn read_dir_entries(dir: &Path) -> DirEntries {
    match std::fs::read_dir(dir) {
      Ok(entries) => {
        let mut map = HashMap::new();
        for entry in entries.flatten() {
          let name = entry.file_name();
          let info = match entry.file_type() {
            Ok(ft) => DirEntryInfo {
              is_file: ft.is_file(),
              is_dir: ft.is_dir(),
              is_symlink: ft.is_symlink(),
            },
            Err(_) => DirEntryInfo { is_file: false, is_dir: false, is_symlink: false },
          };
          map.insert(name, info);
        }
        Some(Arc::new(map))
      }
      Err(_) => None,
    }
  }

  /// Get directory entries. First checks if the path is known to not be a directory
  /// (from its parent's cache), then reads from disk if necessary.
  fn get_dir_entries(&self, dir: &Path) -> DirEntries {
    // Fast path: already cached
    if let Some(cached) = self.dir_cache.get(dir) {
      return cached.value().clone();
    }

    // Check if parent cache knows this path is not a directory.
    // This avoids ~10k failed openat calls for the tsconfig walk pattern.
    // We only do a read (get) on the parent cache — no write lock, no recursion.
    if let (Some(parent), Some(dir_name)) = (dir.parent(), dir.file_name()) {
      if let Some(parent_cache) = self.dir_cache.get(parent) {
        let should_skip = match parent_cache.value() {
          Some(entries) => {
            match entries.get(dir_name) {
              Some(info) => !info.is_dir && !info.is_symlink, // file, not dir
              None => true,                                   // doesn't exist in parent listing
            }
          }
          None => true, // parent not a dir
        };
        // Drop the parent cache reference before inserting into dir_cache
        drop(parent_cache);

        if should_skip {
          self.dir_cache.insert(dir.to_path_buf(), None);
          return None;
        }
      }
      // If parent isn't cached yet, we need to cache it first.
      // Read parent directory to populate cache.
      if !self.dir_cache.contains_key(parent) {
        let parent_entries = Self::read_dir_entries(parent);
        self.dir_cache.insert(parent.to_path_buf(), parent_entries);

        // Now check again if dir is known to not be a directory
        if let Some(parent_cache) = self.dir_cache.get(parent) {
          let should_skip = match parent_cache.value() {
            Some(entries) => match entries.get(dir_name) {
              Some(info) => !info.is_dir && !info.is_symlink,
              None => true,
            },
            None => true,
          };
          drop(parent_cache);

          if should_skip {
            self.dir_cache.insert(dir.to_path_buf(), None);
            return None;
          }
        }
      }
    }

    // Read directory from disk
    let result = Self::read_dir_entries(dir);
    self.dir_cache.insert(dir.to_path_buf(), result.clone());
    result
  }

  /// Look up metadata from directory cache.
  fn lookup_cached_metadata(&self, path: &Path) -> Option<io::Result<FileMetadata>> {
    let parent = path.parent()?;
    let file_name = path.file_name()?;

    let entries = self.get_dir_entries(parent);

    match entries {
      Some(entries) => {
        if let Some(info) = entries.get(file_name) {
          if info.is_symlink {
            None // metadata() follows symlinks — need real statx
          } else {
            Some(Ok(FileMetadata::new(info.is_file, info.is_dir, false)))
          }
        } else {
          Some(Err(io::Error::new(io::ErrorKind::NotFound, "not found (cached)")))
        }
      }
      None => Some(Err(io::Error::new(io::ErrorKind::NotFound, "parent not a directory (cached)"))),
    }
  }

  /// Look up symlink_metadata from directory cache.
  fn lookup_cached_symlink_metadata(&self, path: &Path) -> Option<io::Result<FileMetadata>> {
    let parent = path.parent()?;
    let file_name = path.file_name()?;

    let entries = self.get_dir_entries(parent);

    match entries {
      Some(entries) => {
        if let Some(info) = entries.get(file_name) {
          Some(Ok(FileMetadata::new(info.is_file, info.is_dir, info.is_symlink)))
        } else {
          Some(Err(io::Error::new(io::ErrorKind::NotFound, "not found (cached)")))
        }
      }
      None => Some(Err(io::Error::new(io::ErrorKind::NotFound, "parent not a directory (cached)"))),
    }
  }
}

impl FileSystem for OsFileSystem {
  fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
    std::fs::remove_dir_all(path)
  }

  fn create_dir_all(&self, path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
  }

  fn write(&self, path: &Path, content: &[u8]) -> io::Result<()> {
    std::fs::write(path, content)
  }

  fn exists(&self, path: &Path) -> bool {
    if let Some(result) = self.lookup_cached_metadata(path) {
      return result.is_ok();
    }
    path.exists()
  }

  fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(path)?;
    let mut paths = Vec::new();
    for entry in entries {
      let entry = entry?;
      paths.push(entry.path());
    }
    Ok(paths)
  }

  fn remove_file(&self, path: &Path) -> io::Result<()> {
    std::fs::remove_file(path)
  }
}

impl OxcResolverFileSystem for OsFileSystem {
  fn new(yarn_pnp: bool) -> Self {
    Self {
      inner: Arc::new(FileSystemOs::new(yarn_pnp)),
      dir_cache: Arc::new(DashMap::with_capacity(512)),
    }
  }

  fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
    self.inner.read(path)
  }

  fn read_to_string(&self, path: &Path) -> io::Result<String> {
    self.inner.read_to_string(path)
  }

  fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    if let Some(result) = self.lookup_cached_metadata(path) {
      return result;
    }
    self.inner.metadata(path)
  }

  fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    if let Some(result) = self.lookup_cached_symlink_metadata(path) {
      return result;
    }
    self.inner.symlink_metadata(path)
  }

  fn read_link(&self, path: &Path) -> Result<PathBuf, ResolveError> {
    self.inner.read_link(path)
  }

  fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
    self.inner.canonicalize(path)
  }
}
