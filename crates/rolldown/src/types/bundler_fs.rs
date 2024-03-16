use rolldown_fs::FileSystem;

/// You don't need impl this trait manually, it's already implemented for all types that implement `FileSystem + Default + 'static`.
pub trait BundlerFileSystem: FileSystem + Default + 'static {}
impl<T: FileSystem + Default + 'static> BundlerFileSystem for T {}
