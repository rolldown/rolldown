#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use std::ops::{BitAnd, BitOr, BitOrAssign};

use super::{Mask, Simd, traits::BitMask, util::escape_unchecked};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::util::check_cross_page;

const LANES: usize = 32;
const CHUNK: usize = LANES * 4;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Simd256u(__m256i);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Mask256(__m256i);

impl Mask for Mask256 {
    type BitMask = u32;
    type Element = u8;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMask {
        unsafe { _mm256_movemask_epi8(self.0) as u32 }
    }
}

impl BitAnd<Mask256> for Mask256 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask256) -> Self::Output {
        unsafe { Mask256(_mm256_and_si256(self.0, rhs.0)) }
    }
}

impl BitOr<Mask256> for Mask256 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Mask256) -> Self::Output {
        unsafe { Mask256(_mm256_or_si256(self.0, rhs.0)) }
    }
}

impl BitOrAssign<Mask256> for Mask256 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Mask256) {
        unsafe { self.0 = _mm256_or_si256(self.0, rhs.0) }
    }
}

impl Simd for Simd256u {
    const LANES: usize = LANES;
    type Mask = Mask256;
    type Element = u8;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        unsafe { Simd256u(_mm256_loadu_si256(ptr as *const __m256i)) }
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        unsafe { _mm256_storeu_si256(ptr as *mut __m256i, self.0) }
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        unsafe {
            let eq = _mm256_cmpeq_epi8(self.0, rhs.0);
            Mask256(eq)
        }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        unsafe { Simd256u(_mm256_set1_epi8(ch as i8)) }
    }

    #[inline(always)]
    fn le(&self, rhs: &Self) -> Self::Mask {
        unsafe {
            let max = _mm256_max_epu8(self.0, rhs.0);
            let eq = _mm256_cmpeq_epi8(max, rhs.0);
            Mask256(eq)
        }
    }
}

#[inline(always)]
fn escaped_mask(v: Simd256u) -> u32 {
    let x1f = Simd256u::splat(0x1f); // 0x00 ~ 0x20
    let blash = Simd256u::splat(b'\\');
    let quote = Simd256u::splat(b'"');
    let v = v.le(&x1f) | v.eq(&blash) | v.eq(&quote);
    v.bitmask()
}

#[target_feature(enable = "avx2")]
pub unsafe fn format_string(value: &str, dst: &mut [u8]) -> usize {
    unsafe {
        let slice = value.as_bytes();
        let mut sptr = slice.as_ptr();
        let mut dptr = dst.as_mut_ptr();
        let dstart = dptr;
        let mut nb: usize = slice.len();

        *dptr = b'"';
        dptr = dptr.add(1);

        // Process CHUNK (4 * LANES = 128 bytes) at a time
        while nb >= CHUNK {
            // Load 4 SIMD vectors
            let v1 = Simd256u::loadu(sptr);
            let v2 = Simd256u::loadu(sptr.add(LANES));
            let v3 = Simd256u::loadu(sptr.add(LANES * 2));
            let v4 = Simd256u::loadu(sptr.add(LANES * 3));

            // Check all 4 masks
            let mask1 = escaped_mask(v1);
            let mask2 = escaped_mask(v2);
            let mask3 = escaped_mask(v3);
            let mask4 = escaped_mask(v4);

            // Fast path: if all vectors are clean, write the entire chunk
            if mask1.all_zero() && mask2.all_zero() && mask3.all_zero() && mask4.all_zero() {
                v1.storeu(dptr);
                v2.storeu(dptr.add(LANES));
                v3.storeu(dptr.add(LANES * 2));
                v4.storeu(dptr.add(LANES * 3));
                nb -= CHUNK;
                dptr = dptr.add(CHUNK);
                sptr = sptr.add(CHUNK);
            } else {
                // Slow path: handle escape character
                // Process v1
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

                // Process v2
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

                // Process v3
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

                // Process v4
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
            let v = Simd256u::loadu(sptr);
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
                    Simd256u::loadu(placeholder[..].as_ptr())
                }
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                {
                    if check_cross_page(sptr, LANES) {
                        std::ptr::copy_nonoverlapping(sptr, placeholder[..].as_mut_ptr(), nb);
                        Simd256u::loadu(placeholder[..].as_ptr())
                    } else {
                        #[cfg(any(debug_assertions, miri, feature = "asan"))]
                        {
                            std::ptr::copy_nonoverlapping(sptr, placeholder[..].as_mut_ptr(), nb);
                            Simd256u::loadu(placeholder[..].as_ptr())
                        }
                        #[cfg(not(any(debug_assertions, miri)))]
                        {
                            Simd256u::loadu(sptr)
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
