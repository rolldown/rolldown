use super::traits::BitMask;

macro_rules! impl_bits {
    () => {};
    ($($ty:ty)*) => {
        $(
            impl BitMask for $ty {
                const LEN: usize = std::mem::size_of::<$ty>() * 8;

                #[inline]
                fn first_offset(&self) -> usize {
                    self.as_little_endian().trailing_zeros() as usize
                }

                #[inline]
                fn as_little_endian(&self) -> Self {
                    // These bitmasks are logical values (bit `i` == lane `i`),
                    // built either by `Mask128::bitmask` (portable path) or by
                    // x86 movemask intrinsics, never by reinterpreting memory.
                    // Byte-swapping on big-endian was therefore wrong: it broke
                    // the portable path on s390x, while being a harmless no-op on
                    // the x86-only SIMD paths (which are always little-endian).
                    self.clone()
                }

                #[inline]
                fn all_zero(&self) -> bool {
                    *self == 0
                }

                #[inline]
                fn clear_high_bits(&self, n: usize) -> Self {
                    debug_assert!(n <= Self::LEN);
                    *self & ((u64::MAX as $ty) >> n)
                }
            }
        )*
    };
}

impl_bits!(u16 u32 u64);

#[cfg(target_arch = "aarch64")]
/// Use u64 representation the bitmask of Neon vector.
///         (low)
/// Vector: 00-ff-ff-ff-ff-00-00-00
/// Mask  : 0000-1111-1111-1111-1111-0000-0000-0000
///
/// first_offset() = 1
/// clear_high_bits(4) = Mask(0000-1111-1111-1111-[0000]-0000-0000-0000)
///
/// reference: https://community.arm.com/arm-community-blogs/b/infrastructure-solutions-blog/posts/porting-x86-vector-bitmask-optimizations-to-arm-neon
pub struct NeonBits(u64);

#[cfg(target_arch = "aarch64")]
impl NeonBits {
    #[inline]
    pub fn new(u: u64) -> Self {
        Self(u)
    }
}

#[cfg(target_arch = "aarch64")]
impl BitMask for NeonBits {
    const LEN: usize = 16;

    #[inline]
    fn first_offset(&self) -> usize {
        (self.as_little_endian().0.trailing_zeros() as usize) >> 2
    }

    #[inline]
    fn as_little_endian(&self) -> Self {
        #[cfg(target_endian = "little")]
        {
            Self::new(self.0)
        }
        #[cfg(target_endian = "big")]
        {
            Self::new(self.0.swap_bytes())
        }
    }

    #[inline]
    fn all_zero(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    fn clear_high_bits(&self, n: usize) -> Self {
        debug_assert!(n <= Self::LEN);
        Self(self.0 & u64::MAX >> (n * 4))
    }
}
