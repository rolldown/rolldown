#![allow(non_camel_case_types)]

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
pub(crate) mod avx2;
#[cfg(all(any(target_arch = "x86_64", target_arch = "x86"), feature = "avx512"))]
pub(crate) mod avx512;
pub mod bits;
#[cfg(target_arch = "aarch64")]
pub(crate) mod neon;
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
pub(crate) mod sse2;
mod traits;
mod util;
pub(crate) mod v128;

pub use self::traits::{Mask, Simd};
