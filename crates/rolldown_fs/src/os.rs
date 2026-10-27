use std::{
  fmt, io,
  path::{Path, PathBuf},
  sync::Arc,
};

use oxc_resolver::{FileMetadata, FileSystem as OxcResolverFileSystem, FileSystemOs, ResolveError};

use crate::{file_system::FileSystem, profile as P};
use std::time::Instant;

/// Operating System
#[derive(Clone)]
pub struct OsFileSystem(Arc<FileSystemOs>);

impl fmt::Debug for OsFileSystem {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "OsFileSystem")
  }
}

impl FileSystem for OsFileSystem {
  fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
    std::fs::remove_dir_all(path)
  }

  fn create_dir_all(&self, path: &Path) -> io::Result<()> {
    let __t = Instant::now();
    let __r = std::fs::create_dir_all(path);
    P::OTHER.add(__t, 0);
    P::record_any_interval(__t);
    __r
  }

  fn write(&self, path: &Path, content: &[u8]) -> io::Result<()> {
    let __t = Instant::now();
    let __r = std::fs::write(path, content);
    P::WRITE.add(__t, content.len());
    P::record_any_interval(__t);
    __r
  }

  fn exists(&self, path: &Path) -> bool {
    let __t = Instant::now();
    let __r = path.exists();
    P::OTHER.add(__t, 0);
    P::record_any_interval(__t);
    __r
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
    Self(Arc::new(FileSystemOs::new(yarn_pnp)))
  }

  fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
    if P::dircache_says_missing(path) {
      return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    if P::content_cache_enabled() {
      if let Some(v) = P::content_cache_get_bytes(path) {
        return Ok(v);
      }
      let r = P::content_cache_read_bytes(path);
      if matches!(&r, Err(e) if e.kind() == io::ErrorKind::NotFound) {
        P::dircache_note_missing(path);
      }
      return r;
    }
    let __t = Instant::now();
    let __r = {
      let _permit = P::fs_permit();
      self.0.read(path)
    };
    P::READ.add(__t, __r.as_ref().map_or(0, Vec::len));
    P::record_any_interval(__t);
    if matches!(&__r, Err(e) if e.kind() == io::ErrorKind::NotFound) {
      P::dircache_note_missing(path);
    }
    __r
  }

  fn read_to_string(&self, path: &Path) -> io::Result<String> {
    if P::cache_enabled() {
      if let Some(s) = P::cache_get(path) {
        return Ok(s);
      }
    }
    let __t = Instant::now();
    let __r = {
      let _permit = P::fs_permit();
      self.0.read_to_string(path)
    };
    P::READ_TO_STRING.add(__t, __r.as_ref().map_or(0, String::len));
    P::record_any_interval(__t);
    if P::cache_enabled() {
      if let Ok(s) = &__r {
        P::cache_put(path, s);
      }
    }
    __r
  }

  fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    let __t = Instant::now();
    let __r = {
      let _permit = P::fs_permit();
      self.0.metadata(path)
    };
    P::METADATA.add(__t, 0);
    P::record_any_interval(__t);
    __r
  }

  fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    if let Some(m) = P::prefetch_get_meta(path) {
      return m.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound));
    }
    if P::dircache_enabled() {
      let __t = Instant::now();
      let __r = P::dircache_symlink_metadata(path);
      P::SYMLINK_METADATA.add(__t, 0);
      return __r;
    }
    if P::meta_cache_enabled() {
      if let Some(m) = P::meta_get(path) {
        return m.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound));
      }
      let __r = {
        let _permit = P::fs_permit();
        self.0.symlink_metadata(path)
      };
      P::meta_put(path, __r.as_ref().ok().copied());
      return __r;
    }
    let __t = Instant::now();
    let __r = {
      let _permit = P::fs_permit();
      self.0.symlink_metadata(path)
    };
    P::SYMLINK_METADATA.add(__t, 0);
    P::record_any_interval(__t);
    __r
  }

  fn read_link(&self, path: &Path) -> Result<PathBuf, ResolveError> {
    let __t = Instant::now();
    let __r = {
      let _permit = P::fs_permit();
      self.0.read_link(path)
    };
    P::READ_LINK.add(__t, 0);
    P::record_any_interval(__t);
    __r
  }

  fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
    let __t = Instant::now();
    let __r = {
      let _permit = P::fs_permit();
      self.0.canonicalize(path)
    };
    P::CANONICALIZE.add(__t, 0);
    P::record_any_interval(__t);
    __r
  }
}
