#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use std::ops::{BitAnd, BitOr, BitOrAssign};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::util::check_cross_page;
use super::{Mask, Simd, traits::BitMask, util::escape_unchecked};

const LANES: usize = 64;
const CHUNK: usize = LANES * 4;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Simd512u(__m512i);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Mask512(__mmask64);

impl Mask for Mask512 {
    type BitMask = u64;
    type Element = u8;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMask {
        self.0
    }
}

impl BitOr for Mask512 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        Mask512(self.0 | rhs.0)
    }
}

impl BitOrAssign for Mask512 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd<Mask512> for Mask512 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask512) -> Self::Output {
        Mask512(self.0 & rhs.0)
    }
}

impl Simd for Simd512u {
    const LANES: usize = LANES;
    type Element = u8;
    type Mask = Mask512;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        unsafe { Simd512u(_mm512_loadu_si512(ptr as *const __m512i)) }
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        unsafe { _mm512_storeu_si512(ptr as *mut __m512i, self.0) }
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        unsafe { Mask512(_mm512_cmpeq_epi8_mask(self.0, rhs.0)) }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        unsafe { Simd512u(_mm512_set1_epi8(ch as i8)) }
    }

    #[inline(always)]
    fn le(&self, rhs: &Self) -> Self::Mask {
        unsafe { Mask512(_mm512_cmple_epu8_mask(self.0, rhs.0)) }
    }
}

#[inline(always)]
fn escaped_mask(v: Simd512u) -> u64 {
    let x1f = Simd512u::splat(0x1f); // 0x00 ~ 0x20
    let blash = Simd512u::splat(b'\\');
    let quote = Simd512u::splat(b'"');
    let v = v.le(&x1f) | v.eq(&blash) | v.eq(&quote);
    v.bitmask()
}

#[target_feature(enable = "avx512f")]
pub unsafe fn format_string(value: &str, dst: &mut [u8]) -> usize {
    unsafe {
        let slice = value.as_bytes();
        let mut sptr = slice.as_ptr();
        let mut dptr = dst.as_mut_ptr();
        let dstart = dptr;
        let mut nb: usize = slice.len();

        *dptr = b'"';
        dptr = dptr.add(1);

        // Process CHUNK (4 * LANES = 256 bytes) at a time
        while nb >= CHUNK {
            // Load 4 SIMD vectors
            let v1 = Simd512u::loadu(sptr);
            let v2 = Simd512u::loadu(sptr.add(LANES));
            let v3 = Simd512u::loadu(sptr.add(LANES * 2));
            let v4 = Simd512u::loadu(sptr.add(LANES * 3));

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
            let v = Simd512u::loadu(sptr);
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
        let mut v;
        while nb > 0 {
            v = {
                #[cfg(not(any(target_os = "linux", target_os = "macos")))]
                {
                    std::ptr::copy_nonoverlapping(sptr, placeholder[..].as_mut_ptr(), nb);
                    Simd512u::loadu(placeholder[..].as_ptr())
                }
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                {
                    if check_cross_page(sptr, LANES) {
                        std::ptr::copy_nonoverlapping(sptr, placeholder[..].as_mut_ptr(), nb);
                        Simd512u::loadu(placeholder[..].as_ptr())
                    } else {
                        #[cfg(any(debug_assertions, miri, feature = "asan"))]
                        {
                            std::ptr::copy_nonoverlapping(sptr, placeholder[..].as_mut_ptr(), nb);
                            Simd512u::loadu(placeholder[..].as_ptr())
                        }
                        #[cfg(not(any(debug_assertions, miri)))]
                        {
                            Simd512u::loadu(sptr)
                        }
                    }
                }
            };

            v.storeu(std::slice::from_raw_parts_mut(dptr, LANES).as_mut_ptr());
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
