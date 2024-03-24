impl FileSystem for Box<dyn FileSystem> {
  fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
    self.as_ref().remove_dir_all(path)
  }

  fn create_dir_all(&self, path: &Path) -> io::Result<()> {
    self.as_ref().create_dir_all(path)
  }

  fn write(&self, path: &Path, content: &[u8]) -> io::Result<()> {
    self.as_ref().write(path, content)
  }

  fn exists(&self, path: &Path) -> bool {
    self.as_ref().exists(path)
  }
}

impl oxc_resolver::FileSystem for Box<dyn FileSystem> {
  fn read_to_string(&self, path: &Path) -> io::Result<String> {
    self.as_ref().read_to_string(path)
  }

  fn metadata(&self, path: &Path) -> io::Result<oxc_resolver::FileMetadata> {
    self.as_ref().metadata(path)
  }

  fn symlink_metadata(&self, path: &Path) -> io::Result<oxc_resolver::FileMetadata> {
    self.as_ref().symlink_metadata(path)
  }

  fn canonicalize(&self, path: &Path) -> io::Result<std::path::PathBuf> {
    self.as_ref().canonicalize(path)
  }
}
