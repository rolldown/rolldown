use std::ops::{BitAnd, BitOr, BitOrAssign};

use crate::simd::traits::BitMask;

use super::{Mask, Simd, util::escape_unchecked};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::util::check_cross_page;

const LANES: usize = 16;

#[derive(Debug)]
pub struct Simd128u([u8; 16]);

#[derive(Debug)]
pub struct Mask128(pub(crate) [u8; 16]);

impl Simd for Simd128u {
    type Element = u8;
    const LANES: usize = 16;
    type Mask = Mask128;

    unsafe fn loadu(ptr: *const u8) -> Self {
        let v = unsafe { std::slice::from_raw_parts(ptr, Self::LANES) };
        let mut res = [0u8; 16];
        res.copy_from_slice(v);
        Self(res)
    }

    unsafe fn storeu(&self, ptr: *mut u8) {
        let data = &self.0;
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, Self::LANES) };
    }

    fn eq(&self, rhs: &Self) -> Self::Mask {
        let mut mask = [0u8; 16];
        for (i, item) in mask.iter_mut().enumerate().take(Self::LANES) {
            *item = if self.0[i] == rhs.0[i] { 1 } else { 0 };
        }
        Mask128(mask)
    }

    fn splat(value: u8) -> Self {
        Self([value; Self::LANES])
    }

    fn le(&self, rhs: &Self) -> Self::Mask {
        let mut mask = [0u8; 16];
        for (i, item) in mask.iter_mut().enumerate().take(Self::LANES) {
            *item = if self.0[i] <= rhs.0[i] { 1 } else { 0 };
        }
        Mask128(mask)
    }
}

impl Mask for Mask128 {
    type BitMask = u16;
    type Element = u8;

    fn bitmask(self) -> Self::BitMask {
        // The mask is a logical bitmask (lane `i` -> bit `i`), not a
        // reinterpretation of memory, so its layout must not depend on target
        // endianness. The previous big-endian branch placed lane `i` at bit
        // `15 - i`, which was inconsistent with `BitMask::first_offset` (which
        // reads the *lowest* set bit). On big-endian targets such as s390x that
        // mismatch caused offset underflow ("attempt to subtract with overflow")
        // and corrupted JSON string escaping.
        self.0
            .iter()
            .enumerate()
            .fold(0, |acc, (i, &b)| acc | ((b as u16) << i))
    }
}

impl BitAnd for Mask128 {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        let mut result = [0u8; 16];
        for (i, item) in result.iter_mut().enumerate() {
            *item = self.0[i] & rhs.0[i];
        }
        Mask128(result)
    }
}

impl BitOr for Mask128 {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        let mut result = [0u8; 16];
        for (i, item) in result.iter_mut().enumerate() {
            *item = self.0[i] | rhs.0[i];
        }
        Mask128(result)
    }
}

impl BitOrAssign for Mask128 {
    fn bitor_assign(&mut self, rhs: Self) {
        for i in 0..16 {
            self.0[i] |= rhs.0[i];
        }
    }
}

#[inline(always)]
fn escaped_mask(v: Simd128u) -> u16 {
    let x1f = Simd128u::splat(0x1f); // 0x00 ~ 0x20
    let blash = Simd128u::splat(b'\\');
    let quote = Simd128u::splat(b'"');
    let v = v.le(&x1f) | v.eq(&blash) | v.eq(&quote);
    v.bitmask()
}

pub fn format_string(value: &str, dst: &mut [u8]) -> usize {
    unsafe {
        let slice = value.as_bytes();
        let mut sptr = slice.as_ptr();
        let mut dptr = dst.as_mut_ptr();
        let dstart = dptr;
        let mut nb: usize = slice.len();

        *dptr = b'"';
        dptr = dptr.add(1);

        // Main loop: process LANES bytes at a time
        while nb >= LANES {
            let v = Simd128u::loadu(sptr);
            v.storeu(dptr);
            let mask = escaped_mask(v);

            if mask == 0 {
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
                    if check_cross_page(sptr, Simd128u::LANES) {
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
            let mut mask = escaped_mask(v);
            // Clear high bits for partial vector
            mask &= (1u16 << nb) - 1;

            if mask == 0 {
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
