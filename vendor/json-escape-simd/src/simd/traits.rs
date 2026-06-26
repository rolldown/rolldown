use std::ops::{BitAnd, BitOr, BitOrAssign};

/// Portable SIMD traits
pub trait Simd: Sized {
    const LANES: usize;

    type Element;
    type Mask: Mask;

    /// # Safety
    unsafe fn loadu(ptr: *const u8) -> Self;

    /// # Safety
    unsafe fn storeu(&self, ptr: *mut u8);

    fn eq(&self, rhs: &Self) -> Self::Mask;

    fn splat(elem: Self::Element) -> Self;

    /// less or equal
    fn le(&self, rhs: &Self) -> Self::Mask;
}

/// Portable SIMD mask traits
pub trait Mask: Sized + BitOr<Self> + BitOrAssign + BitAnd<Self> {
    type Element;
    type BitMask: BitMask;

    fn bitmask(self) -> Self::BitMask;
}

/// Trait for the bitmask of a vector Mask.
pub trait BitMask {
    /// Total bits in the bitmask.
    const LEN: usize;

    /// get the offset of the first `1` bit.
    fn first_offset(&self) -> usize;

    /// convert bitmask as little endian
    fn as_little_endian(&self) -> Self;

    /// whether all bits are zero.
    fn all_zero(&self) -> bool;

    /// clear high n bits.
    fn clear_high_bits(&self, n: usize) -> Self;
}
