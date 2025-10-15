mod file_system;
#[cfg(feature = "memory")]
mod memory;
#[cfg(feature = "memory")]
pub use memory::MemoryFileSystem;
#[cfg(feature = "os")]
mod os;
pub use crate::file_system::{FileSystem, FileSystemUtils};
#[cfg(feature = "os")]
pub use os::OsFileSystem;
pub use oxc_resolver::FileSystem as OxcResolverFileSystem;
