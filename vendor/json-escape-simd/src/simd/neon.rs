use std::arch::aarch64::*;

use super::{Mask, Simd, bits::NeonBits, traits::BitMask, util::escape_unchecked};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::util::check_cross_page;

const LANES: usize = 16;
const CHUNK: usize = LANES * 4;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Simd128u(uint8x16_t);

impl Simd for Simd128u {
    const LANES: usize = LANES;
    type Mask = Mask128;
    type Element = u8;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        unsafe { Self(vld1q_u8(ptr)) }
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        unsafe { vst1q_u8(ptr, self.0) };
    }

    #[inline(always)]
    fn eq(&self, lhs: &Self) -> Self::Mask {
        unsafe { Mask128(vceqq_u8(self.0, lhs.0)) }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        unsafe { Self(vdupq_n_u8(ch)) }
    }

    // less or equal
    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        unsafe { Mask128(vcleq_u8(self.0, lhs.0)) }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Mask128(pub(crate) uint8x16_t);

impl Mask for Mask128 {
    type BitMask = NeonBits;
    type Element = u8;

    /// Convert Mask Vector 0x00-ff-ff to Bits 0b0000-1111-1111
    /// Reference: https://community.arm.com/arm-community-blogs/b/infrastructure-solutions-blog/posts/porting-x86-vector-bitmask-optimizations-to-arm-neon
    #[inline(always)]
    fn bitmask(self) -> Self::BitMask {
        unsafe {
            let v16 = vreinterpretq_u16_u8(self.0);
            let sr4 = vshrn_n_u16(v16, 4);
            let v64 = vreinterpret_u64_u8(sr4);
            NeonBits::new(vget_lane_u64(v64, 0))
        }
    }
}

// Bitwise AND for Mask128
impl std::ops::BitAnd<Mask128> for Mask128 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask128) -> Self::Output {
        unsafe { Self(vandq_u8(self.0, rhs.0)) }
    }
}

// Bitwise OR for Mask128
impl std::ops::BitOr<Mask128> for Mask128 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Mask128) -> Self::Output {
        unsafe { Self(vorrq_u8(self.0, rhs.0)) }
    }
}

// Bitwise OR assignment for Mask128
impl std::ops::BitOrAssign<Mask128> for Mask128 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Mask128) {
        unsafe {
            self.0 = vorrq_u8(self.0, rhs.0);
        }
    }
}

/// Returns the vector-domain escape mask (Mask128) without extracting to bitmask.
/// This allows combining multiple masks with SIMD OR before a single bitmask extraction.
#[inline(always)]
fn escaped_mask_vec(v: Simd128u) -> Mask128 {
    let x1f = Simd128u::splat(0x1f); // 0x00 ~ 0x1f
    let blash = Simd128u::splat(b'\\');
    let quote = Simd128u::splat(b'"');
    v.le(&x1f) | v.eq(&blash) | v.eq(&quote)
}

#[inline(always)]
fn escaped_mask(v: Simd128u) -> NeonBits {
    escaped_mask_vec(v).bitmask()
}

#[target_feature(enable = "neon")]
pub unsafe fn format_string(value: &str, dst: &mut [u8]) -> usize {
    unsafe {
        let slice = value.as_bytes();
        let mut sptr = slice.as_ptr();
        let mut dptr = dst.as_mut_ptr();
        let dstart = dptr;
        let mut nb: usize = slice.len();

        *dptr = b'"';
        dptr = dptr.add(1);

        // Process CHUNK (4 * LANES = 64 bytes) at a time
        while nb >= CHUNK {
            // Load 4 SIMD vectors
            let v1 = Simd128u::loadu(sptr);
            let v2 = Simd128u::loadu(sptr.add(LANES));
            let v3 = Simd128u::loadu(sptr.add(LANES * 2));
            let v4 = Simd128u::loadu(sptr.add(LANES * 3));

            // Compute escape masks in vector domain (all independent, can pipeline)
            let m1 = escaped_mask_vec(v1);
            let m2 = escaped_mask_vec(v2);
            let m3 = escaped_mask_vec(v3);
            let m4 = escaped_mask_vec(v4);

            // Combined check: single bitmask extraction instead of four
            if (m1 | m2 | m3 | m4).bitmask().all_zero() {
                v1.storeu(dptr);
                v2.storeu(dptr.add(LANES));
                v3.storeu(dptr.add(LANES * 2));
                v4.storeu(dptr.add(LANES * 3));
                nb -= CHUNK;
                dptr = dptr.add(CHUNK);
                sptr = sptr.add(CHUNK);
            } else {
                // Slow path: extract individual bitmasks lazily
                let mask1 = m1.bitmask();
                v1.storeu(dptr);
                if !mask1.all_zero() {
                    let cn = mask1.first_offset();
                    nb -= cn;
                    dptr = dptr.add(cn);
                    sptr = sptr.add(cn);
                    escape_unchecked(&mut sptr, &mut nb, &mut dptr);
                    continue;
                }
                nb -= LANES;
                dptr = dptr.add(LANES);
                sptr = sptr.add(LANES);

                let mask2 = m2.bitmask();
                v2.storeu(dptr);
                if !mask2.all_zero() {
                    let cn = mask2.first_offset();
                    nb -= cn;
                    dptr = dptr.add(cn);
                    sptr = sptr.add(cn);
                    escape_unchecked(&mut sptr, &mut nb, &mut dptr);
                    continue;
                }
                nb -= LANES;
                dptr = dptr.add(LANES);
                sptr = sptr.add(LANES);

                let mask3 = m3.bitmask();
                v3.storeu(dptr);
                if !mask3.all_zero() {
                    let cn = mask3.first_offset();
                    nb -= cn;
                    dptr = dptr.add(cn);
                    sptr = sptr.add(cn);
                    escape_unchecked(&mut sptr, &mut nb, &mut dptr);
                    continue;
                }
                nb -= LANES;
                dptr = dptr.add(LANES);
                sptr = sptr.add(LANES);

                let mask4 = m4.bitmask();
                v4.storeu(dptr);
                if !mask4.all_zero() {
                    let cn = mask4.first_offset();
                    nb -= cn;
                    dptr = dptr.add(cn);
                    sptr = sptr.add(cn);
                    escape_unchecked(&mut sptr, &mut nb, &mut dptr);
                    continue;
                }
                nb -= LANES;
                dptr = dptr.add(LANES);
                sptr = sptr.add(LANES);
            }
        }

        // Process remaining LANES bytes at a time
        while nb >= LANES {
            let v = Simd128u::loadu(sptr);
            v.storeu(dptr);
            let mask = escaped_mask(v);

            if mask.all_zero() {
                nb -= LANES;
                dptr = dptr.add(LANES);
                sptr = sptr.add(LANES);
            } else {
                let cn = mask.first_offset();
                nb -= cn;
                dptr = dptr.add(cn);
                sptr = sptr.add(cn);
                escape_unchecked(&mut sptr, &mut nb, &mut dptr);
            }
        }

        // Handle remaining bytes
        let mut placeholder: [u8; LANES] = [0; LANES];
        while nb > 0 {
            let v = {
                #[cfg(not(any(target_os = "linux", target_os = "macos")))]
                {
                    std::ptr::copy_nonoverlapping(sptr, placeholder[..].as_mut_ptr(), nb);
                    Simd128u::loadu(placeholder[..].as_ptr())
                }
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                {
                    if check_cross_page(sptr, LANES) {
                        std::ptr::copy_nonoverlapping(sptr, placeholder[..].as_mut_ptr(), nb);
                        Simd128u::loadu(placeholder[..].as_ptr())
                    } else {
                        #[cfg(any(debug_assertions, miri, feature = "asan"))]
                        {
                            std::ptr::copy_nonoverlapping(sptr, placeholder[..].as_mut_ptr(), nb);
                            Simd128u::loadu(placeholder[..].as_ptr())
                        }
                        #[cfg(not(any(debug_assertions, miri)))]
                        {
                            Simd128u::loadu(sptr)
                        }
                    }
                }
            };

            v.storeu(dptr);
            let mask = escaped_mask(v).clear_high_bits(LANES - nb);

            if mask.all_zero() {
                dptr = dptr.add(nb);
                break;
            } else {
                let cn = mask.first_offset();
                nb -= cn;
                dptr = dptr.add(cn);
                sptr = sptr.add(cn);
                escape_unchecked(&mut sptr, &mut nb, &mut dptr);
            }
        }

        *dptr = b'"';
        dptr = dptr.add(1);
        dptr as usize - dstart as usize
    }
}
