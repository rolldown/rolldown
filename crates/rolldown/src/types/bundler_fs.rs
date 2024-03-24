use rolldown_fs::FileSystem;

/// You don't need impl this trait manually, it's already implemented for all types that implement `FileSystem + Default + Clone + 'static`.
///
/// ## Notice on `Clone` constraint
///
/// Rolldown will access the passing file system from multiple places, which means rolldown will clone the file system multiple times.
/// so it's important to make sure these file system is unique. Rolldown could wrap the file system with `Arc`, but it will
/// cause unnecessary overhead while using OS file system. So, it's your responsibility to make sure that the file system
/// is unique after cloning.
pub trait BundlerFileSystem: FileSystem + Default + Clone + 'static {}
impl<T: FileSystem + Default + Clone + 'static> BundlerFileSystem for T {}
